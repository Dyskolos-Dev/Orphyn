//! Server clock estimation for SyncPlay. Each `/GetUtcTime` round trip gives one NTP-style sample:
//!
//! ```text
//! offset = ((received - requestSent) + (sent - responseReceived)) / 2   (server − local)
//! delay  = (responseReceived - requestSent) - (sent - received)          (network round trip)
//! ```
//!
//! The samples with the smallest delay are the most trustworthy, so the estimate is the median
//! offset of the best ones.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSample {
    /// Server time minus local time (ms).
    pub offset: f64,
    /// Round-trip network delay (ms).
    pub delay: f64,
}

pub fn time_sample(
    request_sent: f64,
    server_received: f64,
    server_sent: f64,
    response_received: f64,
) -> TimeSample {
    TimeSample {
        offset: (server_received - request_sent + (server_sent - response_received)) / 2.0,
        delay: (response_received - request_sent - (server_sent - server_received)).max(0.0),
    }
}

pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    }
}

#[derive(Debug, Clone)]
pub struct ClockOffsetEstimator {
    samples: Vec<TimeSample>,
    max_samples: usize,
    best_fraction: f64,
}

impl Default for ClockOffsetEstimator {
    fn default() -> Self {
        Self::new(16, 0.5)
    }
}

impl ClockOffsetEstimator {
    pub fn new(max_samples: usize, best_fraction: f64) -> Self {
        Self {
            samples: Vec::new(),
            max_samples,
            best_fraction,
        }
    }

    pub fn add(&mut self, sample: TimeSample) {
        if !sample.offset.is_finite() || !sample.delay.is_finite() {
            return;
        }
        self.samples.push(sample);
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    fn best(&self) -> Vec<TimeSample> {
        let n = ((self.samples.len() as f64 * self.best_fraction).ceil() as usize).max(1);
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.delay.total_cmp(&b.delay));
        sorted.truncate(n);
        sorted
    }

    /// Estimated server − local offset (ms); 0 before the first sample.
    pub fn offset(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        median(&self.best().iter().map(|s| s.offset).collect::<Vec<_>>())
    }

    /// Typical round trip of the good samples (ms).
    pub fn delay(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        median(&self.best().iter().map(|s| s.delay).collect::<Vec<_>>())
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_math() {
        // Server is 100 ms ahead, 20 ms each way, 5 ms processing.
        let s = time_sample(1000.0, 1120.0, 1125.0, 1045.0);
        assert_eq!(s.offset, 100.0);
        assert_eq!(s.delay, 40.0);
    }

    #[test]
    fn median_cases() {
        assert_eq!(median(&[]), 0.0);
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
    }

    #[test]
    fn estimator_prefers_low_delay_and_caps_samples() {
        let mut e = ClockOffsetEstimator::new(4, 0.5);
        assert_eq!(e.offset(), 0.0);
        e.add(TimeSample {
            offset: 100.0,
            delay: 10.0,
        });
        e.add(TimeSample {
            offset: 102.0,
            delay: 12.0,
        });
        e.add(TimeSample {
            offset: 500.0,
            delay: 400.0,
        });
        e.add(TimeSample {
            offset: 900.0,
            delay: 800.0,
        });
        assert_eq!(e.offset(), 101.0);
        assert_eq!(e.delay(), 11.0);
        e.add(TimeSample {
            offset: f64::NAN,
            delay: 1.0,
        });
        e.add(TimeSample {
            offset: 100.0,
            delay: 9.0,
        });
        assert_eq!(e.count(), 4);
        e.reset();
        assert_eq!(e.count(), 0);
    }
}
