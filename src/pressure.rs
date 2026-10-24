use std::{fs, time::Duration};
#[derive(Default)]
pub struct Policy {
    pub paused: bool,
    recovered_since: Option<Duration>,
}
impl Policy {
    pub fn update(&mut self, available: u64, now: Duration) -> Option<bool> {
        if available < 512 * 1024 * 1024 {
            self.recovered_since = None;
            if !self.paused {
                self.paused = true;
                return Some(true);
            }
        } else if self.paused {
            if available >= 1024 * 1024 * 1024 {
                let since = self.recovered_since.get_or_insert(now);
                if now.saturating_sub(*since) >= Duration::from_secs(30) {
                    self.paused = false;
                    self.recovered_since = None;
                    return Some(false);
                }
            } else {
                self.recovered_since = None;
            }
        }
        None
    }
}
pub fn available() -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    text.lines().find_map(|line| {
        line.strip_prefix("MemAvailable:")
            .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
            .map(|v| v.saturating_mul(1024))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_requires_thirty_stable_seconds() {
        let mut p = Policy::default();
        assert_eq!(p.update(1, Duration::ZERO), Some(true));
        assert_eq!(p.update(2 << 30, Duration::from_secs(1)), None);
        assert_eq!(p.update(2 << 30, Duration::from_secs(30)), None);
        assert_eq!(p.update(2 << 30, Duration::from_secs(31)), Some(false));
    }
    #[test]
    fn dip_resets_recovery_clock() {
        let mut p = Policy::default();
        p.update(1, Duration::ZERO);
        p.update(2 << 30, Duration::from_secs(1));
        p.update(800 << 20, Duration::from_secs(25));
        assert_eq!(p.update(2 << 30, Duration::from_secs(35)), None);
        assert_eq!(p.update(2 << 30, Duration::from_secs(64)), None);
        assert_eq!(p.update(2 << 30, Duration::from_secs(65)), Some(false));
    }
}
