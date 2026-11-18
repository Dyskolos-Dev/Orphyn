//! Device discovery: mDNS (Google Cast, AirPlay, RAOP) and SSDP (UPnP MediaRenderer) into a
//! `DeviceRegistry`.

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};
use super::runtime::{spawn, JoinHandle};
use tokio::net::UdpSocket;

use super::description::parse_device_description;
use super::http::fetch_text;
use super::manager::DiscoveryControl;
use super::model::{CastCapabilities, CastDevice, CastKind};
use super::registry::{now_ms, DeviceRegistry};
use super::txt::{
    airplay_device_from_service, cast_device_from_service, AirplaySource, MdnsService,
};
use super::upnp::UpnpEndpoints;

const MEDIA_RENDERER: &str = "urn:schemas-upnp-org:device:MediaRenderer:1";
const RESCAN_MS: u64 = 20_000;
const SWEEP_MS: u64 = 10_000;
const SSDP_ADDR: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Cast,
    Airplay,
    Raop,
}

impl Source {
    fn service_type(self) -> &'static str {
        match self {
            Source::Cast => "_googlecast._tcp.local.",
            Source::Airplay => "_airplay._tcp.local.",
            Source::Raop => "_raop._tcp.local.",
        }
    }
}

const SOURCES: [Source; 3] = [Source::Cast, Source::Airplay, Source::Raop];

struct Running {
    daemon: Option<ServiceDaemon>,
    tasks: Vec<JoinHandle<()>>,
}

#[derive(Default)]
struct Tables {
    /// Known mDNS services by full name, so a rescan can refresh their last-seen time (mdns-sd
    /// reports a service as resolved once, not on every announcement).
    services: HashMap<String, (Source, MdnsService)>,
    airplay_ids: HashSet<String>,
    upnp_endpoints: HashMap<String, UpnpEndpoints>,
    /// SSDP LOCATION -> device id, to skip re-fetching descriptions for known devices.
    locations: HashMap<String, String>,
    inflight: HashSet<String>,
}

pub struct Discovery {
    registry: Arc<DeviceRegistry>,
    enable_ssdp: bool,
    running: Mutex<Option<Running>>,
    tables: Mutex<Tables>,
    me: Weak<Discovery>,
}

fn upnp_device(location: &str, xml: &str) -> Option<(CastDevice, UpnpEndpoints)> {
    let desc = parse_device_description(xml, location)?;
    // Not a usable MediaRenderer without an AVTransport service.
    let av = desc.av_transport_control_url.clone()?;
    let url = reqwest::Url::parse(location).ok()?;
    let host = url.host_str().unwrap_or_default().to_string();
    let id = format!(
        "upnp:{}",
        if desc.udn.is_empty() {
            location
        } else {
            &desc.udn
        }
    );
    let kind_text = format!("{}{}", desc.device_type, desc.model_name).to_lowercase();
    let device = CastDevice {
        id,
        name: if desc.friendly_name.is_empty() {
            host.clone()
        } else {
            desc.friendly_name.clone()
        },
        kind: CastKind::Upnp,
        model: desc.model_name.clone(),
        manufacturer: desc.manufacturer.clone(),
        // DLNA renderers are mostly audio; WiiM/LinkPlay are audio-only streamers.
        capabilities: CastCapabilities {
            audio: true,
            video: !desc.is_wiim
                && ["tv", "video", "display"]
                    .iter()
                    .any(|k| kind_text.contains(k)),
        },
        address: host,
        port: url.port().filter(|p| *p != 0).unwrap_or(80),
        wiim: desc.is_wiim,
    };
    Some((
        device,
        UpnpEndpoints {
            av_transport_control_url: av,
            rendering_control_url: desc.rendering_control_url,
        },
    ))
}

/// Value of an HTTP header (case-insensitive name) in an SSDP response.
pub fn ssdp_header(response: &str, name: &str) -> Option<String> {
    response.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case(name)
            .then(|| v.trim().to_string())
    })
}

fn to_service(info: &mdns_sd::ResolvedService) -> (String, MdnsService) {
    let ty = info.ty_domain.as_str();
    let full = info.fullname.as_str();
    let name = full
        .strip_suffix(ty)
        .unwrap_or(full)
        .trim_end_matches('.')
        .to_string();
    let mut addresses: Vec<String> = info
        .addresses
        .iter()
        .map(|a| a.to_ip_addr().to_string())
        .collect();
    addresses.sort();
    let txt = super::txt::normalize_txt(
        info.txt_properties
            .iter()
            .map(|p| (p.key().to_string(), p.val_str().to_string())),
    );
    (
        full.to_string(),
        MdnsService {
            name,
            host: Some(info.host.trim_end_matches('.').to_string()).filter(|h| !h.is_empty()),
            port: info.port,
            txt,
            addresses,
        },
    )
}

impl Discovery {
    pub fn new(registry: Arc<DeviceRegistry>) -> Arc<Self> {
        Self::with_options(registry, true)
    }

    pub fn with_options(registry: Arc<DeviceRegistry>, enable_ssdp: bool) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            registry,
            enable_ssdp,
            running: Mutex::new(None),
            tables: Mutex::new(Tables::default()),
            me: me.clone(),
        })
    }

    fn tables(&self) -> MutexGuard<'_, Tables> {
        self.tables.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn running(&self) -> MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn is_running(&self) -> bool {
        self.running().is_some()
    }

    pub fn upnp_endpoints(&self, device_id: &str) -> Option<UpnpEndpoints> {
        self.tables().upnp_endpoints.get(device_id).cloned()
    }

    // --- mDNS bookkeeping (sync, unit-tested) --------------------------------------------------

    pub fn on_up(&self, source: Source, fullname: &str, svc: MdnsService, now: u64) {
        let device = match source {
            Source::Cast => cast_device_from_service(&svc),
            Source::Airplay => airplay_device_from_service(&svc, AirplaySource::Airplay),
            Source::Raop => airplay_device_from_service(&svc, AirplaySource::Raop),
        };
        let Some(device) = device else { return };
        {
            let mut t = self.tables();
            t.services.insert(fullname.to_string(), (source, svc));
            match source {
                Source::Airplay => {
                    t.airplay_ids.insert(device.id.clone());
                }
                // _raop only announces an audio receiver: never overwrite the richer _airplay entry.
                Source::Raop if t.airplay_ids.contains(&device.id) => return,
                _ => {}
            }
        }
        self.registry.upsert(device, now);
    }

    pub fn on_down(&self, fullname: &str) {
        let Some((source, svc)) = self.tables().services.remove(fullname) else {
            return;
        };
        let id = match source {
            Source::Cast => cast_device_from_service(&svc).map(|d| d.id),
            Source::Airplay => {
                airplay_device_from_service(&svc, AirplaySource::Airplay).map(|d| d.id)
            }
            Source::Raop => airplay_device_from_service(&svc, AirplaySource::Raop).map(|d| d.id),
        };
        let Some(id) = id else { return };
        {
            let mut t = self.tables();
            match source {
                Source::Airplay => {
                    t.airplay_ids.remove(&id);
                }
                Source::Raop if t.airplay_ids.contains(&id) => return,
                _ => {}
            }
        }
        self.registry.remove(&id);
    }

    /// Refreshes the last-seen time of every mDNS service still on the network.
    pub fn refresh(&self, now: u64) {
        let known: Vec<(String, Source, MdnsService)> = self
            .tables()
            .services
            .iter()
            .map(|(k, (s, v))| (k.clone(), *s, v.clone()))
            .collect();
        for (full, source, svc) in known {
            self.on_up(source, &full, svc, now);
        }
    }

    // --- lifecycle -----------------------------------------------------------------------------

    fn start_mdns(&self, running: &mut Running) {
        let daemon = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                log::error!("[cast] mdns start failed: {e}");
                return;
            }
        };
        for source in SOURCES {
            let rx = match daemon.browse(source.service_type()) {
                Ok(rx) => rx,
                Err(e) => {
                    log::warn!("[cast] mdns browse {} failed: {e}", source.service_type());
                    continue;
                }
            };
            let me = self.me.clone();
            running.tasks.push(spawn(async move {
                while let Ok(event) = rx.recv_async().await {
                    let Some(d) = me.upgrade() else { break };
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let (full, svc) = to_service(&info);
                            d.on_up(source, &full, svc, now_ms());
                        }
                        ServiceEvent::ServiceRemoved(_, full) => d.on_down(&full),
                        _ => {}
                    }
                }
            }));
        }
        running.daemon = Some(daemon);
    }

    async fn ssdp_search(socket: &UdpSocket) {
        let msg = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {MEDIA_RENDERER}\r\n\r\n"
        );
        if let Err(e) = socket.send_to(msg.as_bytes(), SSDP_ADDR).await {
            log::warn!("[cast] ssdp search failed: {e}");
        }
    }

    fn start_ssdp(&self, running: &mut Running) {
        let me = self.me.clone();
        running.tasks.push(spawn(async move {
            let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    log::warn!("[cast] ssdp start failed: {e}");
                    return;
                }
            };
            let _ = socket.set_multicast_ttl_v4(2);
            Self::ssdp_search(&socket).await;
            // Periodic rescan (re-announce queries, refresh mDNS last-seen times).
            let (rescan_me, rescan_socket) = (me.clone(), socket.clone());
            let rescan = spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_millis(RESCAN_MS));
                tick.tick().await;
                loop {
                    tick.tick().await;
                    if let Some(d) = rescan_me.upgrade() {
                        d.refresh(now_ms());
                    }
                    Self::ssdp_search(&rescan_socket).await;
                }
            });
            let mut buf = vec![0u8; 4096];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((n, _)) => {
                        let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                        if let (Some(d), Some(loc)) = (me.upgrade(), ssdp_header(&text, "LOCATION"))
                        {
                            d.on_ssdp_location(loc);
                        }
                    }
                    Err(e) => {
                        log::warn!("[cast] ssdp receive failed: {e}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
                if me.strong_count() == 0 {
                    break;
                }
            }
            rescan.abort();
        }));
    }

    fn on_ssdp_location(&self, location: String) {
        {
            let mut t = self.tables();
            if let Some(known) = t.locations.get(&location).cloned() {
                if self.registry.touch(&known, now_ms()) {
                    return;
                }
            }
            if !t.inflight.insert(location.clone()) {
                return;
            }
        }
        let me = self.me.clone();
        spawn(async move {
            let Some(d) = me.upgrade() else { return };
            if let Err(e) = d.fetch_renderer(&location).await {
                log::warn!("[cast] description fetch failed ({location}): {e}");
            }
            d.tables().inflight.remove(&location);
        });
    }

    async fn fetch_renderer(&self, location: &str) -> Result<(), String> {
        let xml = fetch_text(location, Duration::from_secs(4)).await?;
        if !self.is_running() {
            return Ok(());
        }
        let Some((device, endpoints)) = upnp_device(location, &xml) else {
            return Ok(());
        };
        {
            let mut t = self.tables();
            t.upnp_endpoints.insert(device.id.clone(), endpoints);
            t.locations.insert(location.to_string(), device.id.clone());
        }
        self.registry.upsert(device, now_ms());
        Ok(())
    }
}

impl DiscoveryControl for Discovery {
    fn start(&self) {
        let mut guard = self.running();
        if guard.is_some() {
            return;
        }
        log::info!("[cast] discovery started");
        let mut running = Running {
            daemon: None,
            tasks: Vec::new(),
        };
        self.start_mdns(&mut running);
        if self.enable_ssdp {
            self.start_ssdp(&mut running);
        }
        let me = self.me.clone();
        running.tasks.push(spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(SWEEP_MS));
            tick.tick().await;
            loop {
                tick.tick().await;
                match me.upgrade() {
                    Some(d) => d.registry.sweep(now_ms()),
                    None => break,
                }
            }
        }));
        *guard = Some(running);
    }

    fn stop(&self) {
        let Some(running) = self.running().take() else {
            return;
        };
        log::info!("[cast] discovery stopped");
        for t in running.tasks {
            t.abort();
        }
        if let Some(daemon) = running.daemon {
            if let Err(e) = daemon.shutdown() {
                log::warn!("[cast] mdns shutdown failed: {e}");
            }
        }
        {
            let mut t = self.tables();
            t.services.clear();
            t.airplay_ids.clear();
            t.inflight.clear();
        }
        // Drop anything that expired while running; stale entries must not linger after stop.
        self.registry.sweep(now_ms());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast_svc() -> MdnsService {
        MdnsService {
            name: "Chromecast-abc".into(),
            port: 8009,
            addresses: vec!["192.168.1.20".into()],
            txt: super::super::txt::normalize_txt(
                [
                    ("id", "abc"),
                    ("fn", "Living Room"),
                    ("md", "Chromecast"),
                    ("ca", "5"),
                ]
                .map(|(k, v)| (k.to_string(), v.to_string())),
            ),
            ..Default::default()
        }
    }

    fn setup() -> (Arc<Discovery>, Arc<DeviceRegistry>) {
        let registry = Arc::new(DeviceRegistry::new(Arc::new(|_| {}), 90_000));
        (Discovery::with_options(registry.clone(), false), registry)
    }

    #[test]
    fn keeps_mdns_devices_alive_past_the_registry_ttl_while_they_stay_on_the_network() {
        let (d, registry) = setup();
        // The only "resolved" event the daemon emits for this service.
        d.on_up(
            Source::Cast,
            "Chromecast-abc._googlecast._tcp.local.",
            cast_svc(),
            0,
        );
        assert_eq!(registry.list().len(), 1);
        // Rescans every 20 s refresh the last-seen time from the known services.
        for t in (20_000..=300_000).step_by(20_000) {
            d.refresh(t);
            registry.sweep(t);
        }
        let names: Vec<_> = registry.list().into_iter().map(|x| x.name).collect();
        assert_eq!(names, vec!["Living Room"]);
    }

    #[test]
    fn drops_a_device_that_left_the_network() {
        let (d, registry) = setup();
        let full = "Chromecast-abc._googlecast._tcp.local.";
        d.on_up(Source::Cast, full, cast_svc(), 0);
        d.on_down(full);
        assert!(registry.list().is_empty());
        d.refresh(1000); // nothing left to refresh
        assert!(registry.list().is_empty());
    }

    #[test]
    fn expires_a_device_that_is_no_longer_refreshed() {
        let (d, registry) = setup();
        d.on_up(Source::Cast, "x._googlecast._tcp.local.", cast_svc(), 0);
        registry.sweep(120_000);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn raop_never_overwrites_the_richer_airplay_entry() {
        let (d, registry) = setup();
        let txt = super::super::txt::normalize_txt(
            [
                ("deviceid", "AA:BB:CC:00:11:22"),
                ("features", "0x5A7FFFF7,0x1E"),
                ("model", "AppleTV6,2"),
            ]
            .map(|(k, v)| (k.to_string(), v.to_string())),
        );
        d.on_up(
            Source::Airplay,
            "Apple TV._airplay._tcp.local.",
            MdnsService {
                name: "Apple TV".into(),
                port: 7000,
                addresses: vec!["10.0.0.9".into()],
                txt,
                ..Default::default()
            },
            0,
        );
        d.on_up(
            Source::Raop,
            "AABBCC001122@Apple TV._raop._tcp.local.",
            MdnsService {
                name: "AABBCC001122@Apple TV".into(),
                port: 49152,
                addresses: vec!["10.0.0.9".into()],
                ..Default::default()
            },
            1,
        );
        let list = registry.list();
        assert_eq!(list.len(), 1);
        assert!(list[0].capabilities.video);
        // The raop going away must not remove the airplay device.
        d.on_down("AABBCC001122@Apple TV._raop._tcp.local.");
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn builds_a_upnp_device_from_a_description() {
        let xml = "<root><device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType><friendlyName>Kitchen</friendlyName><manufacturer>Linkplay</manufacturer><modelName>WiiM Pro</modelName><UDN>uuid:1</UDN><serviceList><service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/av</controlURL></service></serviceList></device></root>";
        let (dev, ep) = upnp_device("http://10.0.0.5:49152/desc.xml", xml).unwrap();
        assert_eq!(dev.id, "upnp:uuid:1");
        assert_eq!((dev.address.as_str(), dev.port), ("10.0.0.5", 49152));
        assert!(dev.wiim && dev.capabilities.audio && !dev.capabilities.video);
        assert_eq!(ep.av_transport_control_url, "http://10.0.0.5:49152/av");
        assert!(upnp_device(
            "http://10.0.0.5/d.xml",
            "<root><device><friendlyName>x</friendlyName></device></root>"
        )
        .is_none());
    }

    #[test]
    fn reads_ssdp_headers_case_insensitively() {
        let r = "HTTP/1.1 200 OK\r\nCache-Control: max-age=1800\r\nLocation: http://10.0.0.5:49152/desc.xml\r\nST: x\r\n\r\n";
        assert_eq!(
            ssdp_header(r, "LOCATION").as_deref(),
            Some("http://10.0.0.5:49152/desc.xml")
        );
        assert_eq!(ssdp_header(r, "USN"), None);
    }
}
