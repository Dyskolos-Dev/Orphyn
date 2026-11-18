//! Device construction from mDNS records: Google Cast (`_googlecast._tcp`) and AirPlay
//! (`_airplay._tcp`, `_raop._tcp`).

use std::collections::HashMap;

use super::model::{CastCapabilities, CastDevice, CastKind};

/// A resolved mDNS service, reduced to what the device builders need.
#[derive(Debug, Clone, Default)]
pub struct MdnsService {
    /// Instance name (without the service type).
    pub name: String,
    pub host: Option<String>,
    pub port: u16,
    /// TXT record, keys lower-cased.
    pub txt: HashMap<String, String>,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CastTxt {
    pub id: Option<String>,
    pub fn_: Option<String>,
    pub md: Option<String>,
    /// Capability bitfield.
    pub ca: Option<i64>,
    pub rs: Option<String>,
}

const CA_VIDEO_OUT: i64 = 1;
const CA_AUDIO_OUT: i64 = 4;

/// Lower-cases the keys of a TXT record.
pub fn normalize_txt<I: IntoIterator<Item = (String, String)>>(txt: I) -> HashMap<String, String> {
    txt.into_iter()
        .map(|(k, v)| (k.to_lowercase(), v))
        .collect()
}

pub fn parse_cast_txt(txt: &HashMap<String, String>) -> CastTxt {
    CastTxt {
        id: txt.get("id").cloned(),
        fn_: txt.get("fn").cloned(),
        md: txt.get("md").cloned(),
        ca: txt.get("ca").and_then(|v| v.trim().parse::<i64>().ok()),
        rs: txt.get("rs").cloned(),
    }
}

/// Video capability from the `ca` bit field (bit 0 = video out); otherwise guessed from the model.
pub fn cast_capabilities(txt: &CastTxt) -> CastCapabilities {
    if let Some(ca) = txt.ca {
        let video = ca & CA_VIDEO_OUT != 0;
        let audio = ca & CA_AUDIO_OUT != 0;
        return CastCapabilities {
            audio: audio || video,
            video,
        };
    }
    let md = txt.md.as_deref().unwrap_or("").to_lowercase();
    let audio_only = [
        "audio",
        "home",
        "nest mini",
        "nest audio",
        "nestmini",
        "nestaudio",
        "speaker",
        "group",
        "wiim",
    ]
    .iter()
    .any(|k| md.contains(k));
    CastCapabilities {
        audio: true,
        video: !audio_only,
    }
}

pub fn pick_address(addresses: &[String]) -> Option<String> {
    addresses
        .iter()
        .find(|a| a.parse::<std::net::Ipv4Addr>().is_ok())
        .or_else(|| addresses.first())
        .cloned()
}

pub fn cast_device_from_service(svc: &MdnsService) -> Option<CastDevice> {
    let txt = parse_cast_txt(&svc.txt);
    let address = pick_address(&svc.addresses).or_else(|| svc.host.clone())?;
    let md = txt.md.clone().unwrap_or_else(|| "Google Cast".to_string());
    Some(CastDevice {
        id: format!(
            "chromecast:{}",
            txt.id.clone().unwrap_or_else(|| svc.name.clone())
        ),
        name: txt
            .fn_
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| svc.name.clone()),
        kind: CastKind::Chromecast,
        wiim: {
            let l = md.to_lowercase();
            l.contains("wiim") || l.contains("linkplay")
        },
        model: md,
        manufacturer: "Google".into(),
        capabilities: cast_capabilities(&txt),
        address,
        port: svc.port,
    })
}

/// AirPlay `features` TXT value: `0xLOW,0xHIGH` or a single number, as a 64-bit set.
pub fn parse_airplay_features(value: Option<&str>) -> u64 {
    let Some(value) = value else { return 0 };
    let parse = |s: &str| -> Option<u64> {
        let s = s.trim();
        match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            Some(hex) => u64::from_str_radix(hex, 16).ok(),
            None => s.parse::<u64>().ok(),
        }
    };
    let mut parts = value.split(',');
    let low = parts.next().and_then(parse);
    let high = match parts.next().filter(|p| !p.trim().is_empty()) {
        Some(p) => parse(p),
        None => Some(0),
    };
    match (low, high) {
        (Some(l), Some(h)) => (l & 0xFFFF_FFFF) | (h << 32),
        _ => 0,
    }
}

const FEATURE_VIDEO: u64 = 0; // bit 0: video (URL playback over HTTP)

/// `AA:BB:CC:DD:EE:FF` or `AABBCCDDEEFF@Name` -> `AABBCCDDEEFF`.
pub fn normalize_mac(value: &str) -> String {
    value
        .split('@')
        .next()
        .unwrap_or("")
        .chars()
        .filter(char::is_ascii_hexdigit)
        .collect::<String>()
        .to_uppercase()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirplaySource {
    Airplay,
    Raop,
}

/// Builds a device from `_airplay._tcp` (full support check) or `_raop._tcp` (audio receiver
/// announcement only). Only `_airplay` devices advertising the video/URL-play feature and no
/// password are marked playable; everything else is listed with no capabilities so the UI can
/// explain why it cannot be used.
pub fn airplay_device_from_service(svc: &MdnsService, source: AirplaySource) -> Option<CastDevice> {
    let txt = &svc.txt;
    let address = pick_address(&svc.addresses).or_else(|| svc.host.clone())?;
    let mac_source = match source {
        AirplaySource::Airplay => txt.get("deviceid").unwrap_or(&svc.name),
        AirplaySource::Raop => &svc.name,
    };
    let mac = normalize_mac(mac_source);
    let name = match source {
        AirplaySource::Raop => svc
            .name
            .split_once('@')
            .map_or(svc.name.as_str(), |(_, n)| n)
            .to_string(),
        AirplaySource::Airplay => svc.name.clone(),
    };
    let mut playable = false;
    if source == AirplaySource::Airplay {
        let features = parse_airplay_features(
            txt.get("features")
                .or_else(|| txt.get("ft"))
                .map(String::as_str),
        );
        let needs_password = matches!(txt.get("pw").map(String::as_str), Some("true" | "1"));
        // macOS AirPlay receivers and HomePods advertise the video bit but only speak AirPlay 2
        // (no legacy HTTP /play), so only Apple TV-class receivers are marked playable.
        let model = txt
            .get("model")
            .map(String::as_str)
            .unwrap_or("")
            .to_lowercase();
        let legacy_capable = !["mac", "imac", "audioaccessory", "homepod"]
            .iter()
            .any(|p| model.starts_with(p));
        playable = (features >> FEATURE_VIDEO) & 1 == 1 && !needs_password && legacy_capable;
    }
    Some(CastDevice {
        id: format!(
            "airplay:{}",
            if mac.is_empty() {
                svc.name.clone()
            } else {
                mac
            }
        ),
        name,
        kind: CastKind::Airplay,
        model: txt
            .get("model")
            .or_else(|| txt.get("am"))
            .cloned()
            .unwrap_or_else(|| "AirPlay".into()),
        manufacturer: "Apple".into(),
        capabilities: CastCapabilities {
            audio: playable,
            video: playable,
        },
        address,
        port: if source == AirplaySource::Airplay {
            svc.port
        } else {
            7000
        },
        wiim: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txt(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        normalize_txt(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())))
    }

    #[test]
    fn reads_friendly_name_model_and_capability_flags_from_txt() {
        let svc = MdnsService {
            name: "Chromecast-abc".into(),
            host: Some("abc.local".into()),
            port: 8009,
            addresses: vec!["fe80::1".into(), "192.168.1.20".into()],
            txt: txt(&[
                ("id", "abc"),
                ("FN", "Living Room TV"),
                ("md", "Chromecast"),
                ("ca", "201221"),
            ]),
        };
        let dev = cast_device_from_service(&svc).unwrap();
        assert_eq!(dev.id, "chromecast:abc");
        assert_eq!(dev.name, "Living Room TV");
        assert_eq!(dev.kind, CastKind::Chromecast);
        assert_eq!(dev.model, "Chromecast");
        assert_eq!(dev.address, "192.168.1.20");
        assert_eq!(dev.port, 8009);
        assert!(dev.capabilities.video); // 201221 & 1
    }

    #[test]
    fn treats_ca_without_the_video_bit_as_audio_only_and_falls_back_to_the_model() {
        let caps = |pairs: &[(&str, &str)]| cast_capabilities(&parse_cast_txt(&txt(pairs)));
        assert_eq!(
            caps(&[("ca", "198660")]),
            CastCapabilities {
                audio: true,
                video: false
            }
        );
        assert_eq!(
            caps(&[("md", "Google Home Mini")]),
            CastCapabilities {
                audio: true,
                video: false
            }
        );
        assert_eq!(
            caps(&[("md", "Chromecast Ultra")]),
            CastCapabilities {
                audio: true,
                video: true
            }
        );
    }

    #[test]
    fn needs_an_address() {
        let svc = MdnsService {
            name: "x".into(),
            port: 1,
            ..Default::default()
        };
        assert!(cast_device_from_service(&svc).is_none());
    }

    #[test]
    fn parses_the_airplay_features_bitfield_and_flags_playable_devices() {
        assert_eq!(parse_airplay_features(Some("0x5A7FFFF7,0x1E")) & 1, 1);
        assert_eq!(parse_airplay_features(None), 0);
        assert_eq!(parse_airplay_features(Some("garbage")), 0);
        let tv = airplay_device_from_service(
            &MdnsService {
                name: "Apple TV".into(),
                port: 7000,
                addresses: vec!["10.0.0.9".into()],
                txt: txt(&[
                    ("deviceid", "AA:BB:CC:00:11:22"),
                    ("features", "0x5A7FFFF7,0x1E"),
                    ("model", "AppleTV6,2"),
                ]),
                ..Default::default()
            },
            AirplaySource::Airplay,
        )
        .unwrap();
        assert_eq!(tv.id, "airplay:AABBCC001122");
        assert_eq!(
            tv.capabilities,
            CastCapabilities {
                audio: true,
                video: true
            }
        );
        let raop = airplay_device_from_service(
            &MdnsService {
                name: "AABBCC001122@Apple TV".into(),
                port: 49152,
                addresses: vec!["10.0.0.9".into()],
                ..Default::default()
            },
            AirplaySource::Raop,
        )
        .unwrap();
        assert_eq!(raop.id, tv.id);
        assert_eq!(raop.name, "Apple TV");
        assert_eq!(raop.port, 7000);
        assert_eq!(
            raop.capabilities,
            CastCapabilities {
                audio: false,
                video: false
            }
        );
    }

    #[test]
    fn does_not_mark_mac_or_homepod_receivers_playable() {
        for model in ["MacBookPro17,1", "AudioAccessory5,1"] {
            let d = airplay_device_from_service(
                &MdnsService {
                    name: "x".into(),
                    port: 7000,
                    addresses: vec!["10.0.0.1".into()],
                    txt: txt(&[
                        ("deviceid", "AA:BB:CC:00:11:33"),
                        ("features", "0x5A7FFFF7,0x1E"),
                        ("model", model),
                    ]),
                    ..Default::default()
                },
                AirplaySource::Airplay,
            )
            .unwrap();
            assert_eq!(
                d.capabilities,
                CastCapabilities {
                    audio: false,
                    video: false
                }
            );
        }
    }
}
