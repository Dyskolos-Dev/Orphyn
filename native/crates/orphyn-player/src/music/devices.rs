//! Audio output devices (port of `player/audioDevice.ts`): listing the outputs and working out
//! which one really applies. A device is identified by its name (cpal has no stable id).
use rodio::cpal::traits::{DeviceTrait, HostTrait};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
}

/// The output devices of the system, default one first. Empty when the host has none.
pub fn output_devices() -> Vec<AudioDevice> {
    let host = rodio::cpal::default_host();
    let default = host.default_output_device().and_then(|d| device_name(&d));
    let mut list: Vec<AudioDevice> = host
        .output_devices()
        .map(|it| {
            it.filter_map(|d| device_name(&d))
                .map(|n| AudioDevice {
                    id: n.clone(),
                    label: n,
                })
                .collect()
        })
        .unwrap_or_default();
    list.dedup_by(|a, b| a.id == b.id);
    if let Some(def) = default {
        if let Some(i) = list.iter().position(|d| d.id == def) {
            let d = list.remove(i);
            list.insert(0, d);
        }
    }
    list
}

#[allow(deprecated)]
fn device_name(d: &rodio::cpal::Device) -> Option<String> {
    d.name().ok()
}

/// The device to open: `None` (system default) unless `saved` is currently present. `devices` of
/// `None` means the list is unknown: trust `saved`. (`resolveSinkId`.)
pub fn resolve_device(saved: Option<&str>, devices: Option<&[AudioDevice]>) -> Option<String> {
    let saved = saved.filter(|s| !s.is_empty() && *s != "default" && *s != "communications")?;
    match devices {
        None => Some(saved.to_string()),
        Some(list) => list
            .iter()
            .any(|d| d.id == saved)
            .then(|| saved.to_string()),
    }
}

/// Opens the stream on the named output device, or the default one.
pub(crate) fn open_stream(name: Option<&str>) -> Result<rodio::OutputStream, String> {
    use rodio::OutputStreamBuilder;
    let mut stream = match name.and_then(find_device) {
        Some(dev) => OutputStreamBuilder::from_device(dev)
            .and_then(|b| b.open_stream())
            .or_else(|_| OutputStreamBuilder::open_default_stream()),
        None => OutputStreamBuilder::open_default_stream(),
    }
    .map_err(|e| e.to_string())?;
    stream.log_on_drop(false);
    Ok(stream)
}

fn find_device(name: &str) -> Option<rodio::cpal::Device> {
    rodio::cpal::default_host()
        .output_devices()
        .ok()?
        .find(|d| device_name(d).as_deref() == Some(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(id: &str) -> AudioDevice {
        AudioDevice {
            id: id.into(),
            label: id.into(),
        }
    }

    #[test]
    fn saved_device_applies_only_when_present() {
        let list = [d("Speakers"), d("USB DAC")];
        assert_eq!(resolve_device(None, Some(&list)), None);
        assert_eq!(resolve_device(Some("default"), Some(&list)), None);
        assert_eq!(resolve_device(Some("communications"), None), None);
        assert_eq!(
            resolve_device(Some("USB DAC"), Some(&list)).as_deref(),
            Some("USB DAC")
        );
        // Unplugged: falls back to the default, picked up again when it returns.
        assert_eq!(resolve_device(Some("Gone"), Some(&list)), None);
        // Unknown list: trust the saved choice.
        assert_eq!(resolve_device(Some("Gone"), None).as_deref(), Some("Gone"));
    }
}
