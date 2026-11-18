//! `features/cast/devices.ts`: which devices a picker lists, and how it groups them.
use orphyn_core::cast::CastDevice;

/// What the picker is opened for: audio for the music player, video for the video player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastScope {
    Music,
    Video,
}

/// Devices able to play this kind of media (AirPlay 2 / RAOP-only devices have neither).
pub fn devices_for_scope(devices: &[CastDevice], scope: CastScope) -> Vec<CastDevice> {
    devices
        .iter()
        .filter(|d| match scope {
            CastScope::Video => d.capabilities.video,
            CastScope::Music => d.capabilities.audio,
        })
        .cloned()
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceGroup {
    Speaker,
    Tv,
}

impl DeviceGroup {
    /// Key of the group title in the `cast` namespace.
    pub fn key(self) -> &'static str {
        match self {
            DeviceGroup::Speaker => "cast:group.speaker",
            DeviceGroup::Tv => "cast:group.tv",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            DeviceGroup::Speaker => "speaker",
            DeviceGroup::Tv => "tv",
        }
    }
}

/// Video-capable receivers are TVs, everything else (WiiM included) a speaker.
pub fn group_of(d: &CastDevice) -> DeviceGroup {
    if !d.wiim && d.capabilities.video {
        DeviceGroup::Tv
    } else {
        DeviceGroup::Speaker
    }
}

pub fn group_devices(devices: &[CastDevice]) -> Vec<(DeviceGroup, Vec<CastDevice>)> {
    [DeviceGroup::Speaker, DeviceGroup::Tv]
        .into_iter()
        .filter_map(|g| {
            let mut list: Vec<CastDevice> =
                devices.iter().filter(|d| group_of(d) == g).cloned().collect();
            list.sort_by_key(|d| d.name.to_lowercase());
            (!list.is_empty()).then_some((g, list))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use orphyn_core::cast::{CastCapabilities, CastKind};

    fn dev(name: &str, audio: bool, video: bool, wiim: bool) -> CastDevice {
        CastDevice {
            id: name.into(),
            name: name.into(),
            kind: CastKind::Upnp,
            model: String::new(),
            manufacturer: String::new(),
            capabilities: CastCapabilities { audio, video },
            address: "1.2.3.4".into(),
            port: 80,
            wiim,
        }
    }

    #[test]
    fn scope_filters_by_capability() {
        let all = [dev("tv", true, true, false), dev("spk", true, false, false), dev("raop", false, false, false)];
        assert_eq!(devices_for_scope(&all, CastScope::Video).len(), 1);
        assert_eq!(devices_for_scope(&all, CastScope::Music).len(), 2);
    }

    #[test]
    fn wiim_with_video_is_still_a_speaker_and_groups_sort_by_name() {
        let all = [
            dev("b-tv", true, true, false),
            dev("Zed", true, true, true),
            dev("alpha", true, false, false),
        ];
        let groups = group_devices(&all);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, DeviceGroup::Speaker);
        let names: Vec<&str> = groups[0].1.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Zed"]);
        assert_eq!(groups[1].1[0].name, "b-tv");
    }
}
