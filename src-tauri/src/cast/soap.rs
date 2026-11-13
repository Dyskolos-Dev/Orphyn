//! SOAP over HTTP for UPnP control (AVTransport, RenderingControl).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use super::xml::{escape_xml, find_node, parse_xml};

pub const AVTRANSPORT_SERVICE: &str = "urn:schemas-upnp-org:service:AVTransport:1";
pub const RENDERING_CONTROL_SERVICE: &str = "urn:schemas-upnp-org:service:RenderingControl:1";

/// One SOAP action argument; values are XML-escaped when the envelope is built.
pub type SoapArg = (&'static str, String);

pub fn arg(name: &'static str, value: impl ToString) -> SoapArg {
    (name, value.to_string())
}

/// Builds a SOAP envelope; argument values are XML-escaped (pass DIDL as a plain string).
pub fn build_soap_envelope(service_type: &str, action: &str, args: &[SoapArg]) -> String {
    let body: String = args
        .iter()
        .map(|(k, v)| format!("<{k}>{}</{k}>", escape_xml(v)))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
         <s:Body><u:{action} xmlns:u=\"{service_type}\">{body}</u:{action}></s:Body></s:Envelope>"
    )
}

pub fn soap_action_header(service_type: &str, action: &str) -> String {
    format!("\"{service_type}#{action}\"")
}

/// Parses a SOAP response body into its output arguments; a fault becomes `Err`.
pub fn parse_soap_response(xml: &str) -> Result<HashMap<String, String>, String> {
    let root = parse_xml(xml);
    if let Some(fault) = find_node(&root, "Fault") {
        let code = find_node(fault, "errorCode").map(|n| n.text.trim().to_string());
        let desc = find_node(fault, "errorDescription").map(|n| n.text.trim().to_string());
        let mut msg = String::from("UPnP fault");
        if let Some(c) = code.filter(|c| !c.is_empty()) {
            msg.push(' ');
            msg.push_str(&c);
        }
        if let Some(d) = desc.filter(|d| !d.is_empty()) {
            msg.push_str(": ");
            msg.push_str(&d);
        }
        return Err(msg);
    }
    let mut out = HashMap::new();
    if let Some(response) = find_node(&root, "Body").and_then(|b| b.children.first()) {
        for child in &response.children {
            out.insert(child.name.clone(), child.text.trim().to_string());
        }
    }
    Ok(out)
}

#[derive(Clone, Debug)]
pub struct SoapCall {
    pub control_url: String,
    pub service_type: &'static str,
    pub action: &'static str,
    pub args: Vec<SoapArg>,
}

pub type SoapResult = Result<HashMap<String, String>, String>;
pub type SoapFuture = Pin<Box<dyn Future<Output = SoapResult> + Send>>;
/// Injectable SOAP sender (tests replace it with a mock).
pub type SoapFn = Arc<dyn Fn(SoapCall) -> SoapFuture + Send + Sync>;

/// Sends one SOAP action over HTTP with a hard timeout.
pub async fn soap_call(call: SoapCall) -> SoapResult {
    let payload = build_soap_envelope(call.service_type, call.action, &call.args);
    let resp = super::http::client()
        .post(&call.control_url)
        .timeout(Duration::from_secs(5))
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header(
            "SOAPACTION",
            soap_action_header(call.service_type, call.action),
        )
        .body(payload)
        .send()
        .await
        .map_err(|e| format!("SOAP {} failed: {e}", call.action))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("SOAP {} failed: {e}", call.action))?;
    // Faults arrive with HTTP 500 but a parseable body.
    let out = parse_soap_response(&text)?;
    if status.as_u16() >= 400 {
        return Err(format!("HTTP {} from {}", status.as_u16(), call.action));
    }
    Ok(out)
}

pub fn default_soap() -> SoapFn {
    Arc::new(|c| Box::pin(soap_call(c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cast::didl::{build_didl, tests::rich_media};

    #[test]
    fn builds_an_envelope_with_escaped_arguments_didl_embedded_as_text() {
        let media = rich_media();
        let xml = build_soap_envelope(
            "urn:schemas-upnp-org:service:AVTransport:1",
            "SetAVTransportURI",
            &[
                arg("InstanceID", 0),
                arg("CurrentURI", "http://h/a?x=1&y=2"),
                arg("CurrentURIMetaData", build_didl(&media)),
            ],
        );
        assert!(xml.contains(
            "<u:SetAVTransportURI xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">"
        ));
        assert!(xml.contains("<InstanceID>0</InstanceID>"));
        assert!(xml.contains("<CurrentURI>http://h/a?x=1&amp;y=2</CurrentURI>"));
        assert!(!xml.contains("<DIDL-Lite"));
        assert!(xml.contains("&lt;DIDL-Lite"));
        // Round trip: the embedded metadata decodes back to the DIDL document.
        let root = parse_xml(&xml);
        assert_eq!(
            find_node(&root, "CurrentURIMetaData").unwrap().text,
            build_didl(&media)
        );
        assert_eq!(soap_action_header("urn:s", "Play"), "\"urn:s#Play\"");
    }

    #[test]
    fn parses_responses_and_faults() {
        let ok = parse_soap_response(
            r#"<s:Envelope xmlns:s="x"><s:Body><u:GetPositionInfoResponse xmlns:u="y"><RelTime>0:00:12</RelTime><TrackDuration>0:03:00</TrackDuration></u:GetPositionInfoResponse></s:Body></s:Envelope>"#,
        )
        .unwrap();
        assert_eq!(ok.get("RelTime").map(String::as_str), Some("0:00:12"));
        assert_eq!(ok.get("TrackDuration").map(String::as_str), Some("0:03:00"));
        assert_eq!(ok.len(), 2);
        let fault = r#"<s:Envelope xmlns:s="x"><s:Body><s:Fault><faultstring>UPnPError</faultstring><detail><UPnPError><errorCode>701</errorCode><errorDescription>Transition not available</errorDescription></UPnPError></detail></s:Fault></s:Body></s:Envelope>"#;
        assert_eq!(
            parse_soap_response(fault).unwrap_err(),
            "UPnP fault 701: Transition not available"
        );
    }
}
