//! Live set of discovered devices with last-seen expiry.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use super::model::CastDevice;

pub const DEFAULT_TTL_MS: u64 = 90_000;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

type OnChange = Arc<dyn Fn(Vec<CastDevice>) + Send + Sync>;

pub struct DeviceRegistry {
    devices: Mutex<HashMap<String, (CastDevice, u64)>>,
    on_change: OnChange,
    ttl_ms: u64,
}

fn sorted(map: &HashMap<String, (CastDevice, u64)>) -> Vec<CastDevice> {
    let mut list: Vec<CastDevice> = map.values().map(|(d, _)| d.clone()).collect();
    list.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    list
}

impl DeviceRegistry {
    pub fn new(on_change: OnChange, ttl_ms: u64) -> Self {
        Self {
            devices: Mutex::new(HashMap::new()),
            on_change,
            ttl_ms,
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, (CastDevice, u64)>> {
        self.devices.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn list(&self) -> Vec<CastDevice> {
        sorted(&self.lock())
    }

    pub fn get(&self, id: &str) -> Option<CastDevice> {
        self.lock().get(id).map(|(d, _)| d.clone())
    }

    /// Adds or refreshes a device; notifies only when something visible changed.
    pub fn upsert(&self, device: CastDevice, now: u64) {
        let changed = {
            let mut map = self.lock();
            let prev = map.insert(device.id.clone(), (device.clone(), now));
            prev.is_none_or(|(p, _)| p != device).then(|| sorted(&map))
        };
        if let Some(list) = changed {
            (self.on_change)(list);
        }
    }

    /// Marks a known device as still alive without changing it.
    pub fn touch(&self, id: &str, now: u64) -> bool {
        match self.lock().get_mut(id) {
            Some(e) => {
                e.1 = now;
                true
            }
            None => false,
        }
    }

    pub fn remove(&self, id: &str) {
        let changed = {
            let mut map = self.lock();
            map.remove(id).map(|_| sorted(&map))
        };
        if let Some(list) = changed {
            (self.on_change)(list);
        }
    }

    /// Drops devices not seen within the TTL.
    pub fn sweep(&self, now: u64) {
        let changed = {
            let mut map = self.lock();
            let before = map.len();
            let ttl = self.ttl_ms;
            map.retain(|_, (_, seen)| now.saturating_sub(*seen) <= ttl);
            (map.len() != before).then(|| sorted(&map))
        };
        if let Some(list) = changed {
            (self.on_change)(list);
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::cast::model::{CastCapabilities, CastKind};

    pub fn dev(id: &str, name: &str) -> CastDevice {
        CastDevice {
            id: id.into(),
            name: name.into(),
            kind: CastKind::Upnp,
            model: String::new(),
            manufacturer: String::new(),
            capabilities: CastCapabilities {
                audio: true,
                video: false,
            },
            address: "1.1.1.1".into(),
            port: 80,
            wiim: false,
        }
    }

    #[test]
    fn notifies_on_add_change_remove_and_expires_stale_devices() {
        let changes: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
        let sink = changes.clone();
        let reg = DeviceRegistry::new(
            Arc::new(move |d| {
                sink.lock()
                    .unwrap()
                    .push(d.into_iter().map(|x| x.id).collect())
            }),
            1000,
        );
        reg.upsert(dev("a", "a"), 0);
        reg.upsert(dev("b", "b"), 500);
        reg.upsert(dev("a", "a"), 600); // unchanged: no event
        assert_eq!(*changes.lock().unwrap(), vec![vec!["a"], vec!["a", "b"]]);
        reg.sweep(1200); // a seen at 600 -> alive; none expired
        assert_eq!(changes.lock().unwrap().len(), 2);
        reg.sweep(1550); // b (seen 500) expired, a (seen 600) alive
        assert_eq!(changes.lock().unwrap().last().unwrap(), &vec!["a"]);
        reg.remove("a");
        assert!(changes.lock().unwrap().last().unwrap().is_empty());
    }

    #[test]
    fn touch_keeps_a_known_device_alive() {
        let reg = DeviceRegistry::new(Arc::new(|_| {}), 1000);
        reg.upsert(dev("a", "a"), 0);
        assert!(reg.touch("a", 900));
        assert!(!reg.touch("zzz", 900));
        reg.sweep(1800);
        assert!(reg.get("a").is_some());
    }
}
