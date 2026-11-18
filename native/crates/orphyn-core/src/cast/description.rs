//! UPnP device description parsing.

use super::xml::{find_all, find_node, parse_xml, text_of};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpnpDescription {
    pub udn: String,
    pub friendly_name: String,
    pub manufacturer: String,
    pub model_name: String,
    pub device_type: String,
    /// Absolute AVTransport control URL.
    pub av_transport_control_url: Option<String>,
    /// Absolute RenderingControl control URL.
    pub rendering_control_url: Option<String>,
    pub is_wiim: bool,
}

pub fn is_wiim_manufacturer(manufacturer: &str, model: &str) -> bool {
    let m = manufacturer.to_lowercase();
    m.contains("linkplay") || m.contains("wiim") || model.to_lowercase().contains("wiim")
}

fn resolve_url(path: Option<String>, base: &str) -> Option<String> {
    let path = path.filter(|p| !p.is_empty())?;
    let base = reqwest::Url::parse(base).ok()?;
    base.join(&path).ok().map(|u| u.to_string())
}

/// Parses a description; `location` is the URL it was fetched from (base for relative URLs).
pub fn parse_device_description(xml: &str, location: &str) -> Option<UpnpDescription> {
    let root = parse_xml(xml);
    let device = find_node(&root, "device")?;
    let base = text_of(&root, "URLBase")
        .filter(|b| !b.is_empty())
        .unwrap_or_else(|| location.to_string());
    let manufacturer = text_of(device, "manufacturer").unwrap_or_default();
    let model_name = text_of(device, "modelName").unwrap_or_default();
    let mut result = UpnpDescription {
        udn: text_of(device, "UDN").unwrap_or_default(),
        friendly_name: text_of(device, "friendlyName").unwrap_or_default(),
        is_wiim: is_wiim_manufacturer(&manufacturer, &model_name),
        manufacturer,
        model_name,
        device_type: text_of(device, "deviceType").unwrap_or_default(),
        ..Default::default()
    };
    let mut services = Vec::new();
    find_all(device, "service", &mut services);
    for svc in services {
        let kind = text_of(svc, "serviceType").unwrap_or_default();
        let control = resolve_url(text_of(svc, "controlURL"), &base);
        if kind.contains(":service:AVTransport:") {
            result.av_transport_control_url = control;
        } else if kind.contains(":service:RenderingControl:") {
            result.rendering_control_url = control;
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0"><URLBase>http://192.168.1.50:49152/</URLBase>
<device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
<friendlyName>Salon &amp; Cuisine</friendlyName><manufacturer>Linkplay Technology Inc.</manufacturer>
<modelName>WiiM Mini</modelName><UDN>uuid:abc-123</UDN>
<serviceList>
<service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/upnp/control/rendertransport1</controlURL></service>
<service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>upnp/control/rendercontrol1</controlURL></service>
<service><serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType><controlURL>/cm</controlURL></service>
</serviceList></device></root>"#;

    #[test]
    fn extracts_identity_control_urls_and_flags_wiim() {
        let d = parse_device_description(XML, "http://192.168.1.50:49152/description.xml").unwrap();
        assert_eq!(d.friendly_name, "Salon & Cuisine");
        assert_eq!(d.udn, "uuid:abc-123");
        assert!(d.is_wiim);
        assert_eq!(
            d.av_transport_control_url.as_deref(),
            Some("http://192.168.1.50:49152/upnp/control/rendertransport1")
        );
        assert_eq!(
            d.rendering_control_url.as_deref(),
            Some("http://192.168.1.50:49152/upnp/control/rendercontrol1")
        );
    }

    #[test]
    fn resolves_relative_urls_against_the_location_without_urlbase_and_rejects_junk() {
        let no_base = XML
            .replace("<URLBase>http://192.168.1.50:49152/</URLBase>", "")
            .replace("Linkplay Technology Inc.", "Sonos")
            .replace("WiiM Mini", "One");
        let d =
            parse_device_description(&no_base, "http://10.0.0.2:1400/xml/device_description.xml")
                .unwrap();
        assert!(!d.is_wiim);
        assert_eq!(
            d.av_transport_control_url.as_deref(),
            Some("http://10.0.0.2:1400/upnp/control/rendertransport1")
        );
        assert!(parse_device_description("<html></html>", "http://x/").is_none());
    }
}
