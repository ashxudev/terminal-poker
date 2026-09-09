//! Bounded per-address admission windows, shared across TLS connections.
use std::{
    collections::BTreeMap,
    net::IpAddr,
    time::{Duration, Instant},
};
pub struct Admission {
    entries: BTreeMap<IpAddr, (Instant, u32)>,
    limit: u32,
    window: Duration,
}
impl Admission {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            entries: BTreeMap::new(),
            limit,
            window,
        }
    }
    pub fn allow(&mut self, ip: IpAddr, now: Instant) -> bool {
        self.entries
            .retain(|_, (since, _)| now.saturating_duration_since(*since) < self.window);
        if !self.entries.contains_key(&ip) && self.entries.len() >= 1024 {
            return false;
        }
        let (_, count) = self.entries.entry(ip).or_insert((now, 0));
        if *count >= self.limit {
            return false;
        }
        *count += 1;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn per_address_budget_recovers_and_other_clients_are_independent() {
        let mut a = Admission::new(2, Duration::from_secs(10));
        let now = Instant::now();
        let ip = "192.168.1.2".parse().unwrap();
        assert!(a.allow(ip, now));
        assert!(a.allow(ip, now));
        assert!(!a.allow(ip, now));
        assert!(a.allow("192.168.1.3".parse().unwrap(), now));
        assert!(a.allow(ip, now + Duration::from_secs(10)));
    }
    #[test]
    fn address_tracking_is_bounded() {
        let mut a = Admission::new(1, Duration::from_secs(10));
        let now = Instant::now();
        for n in 1..=1024u32 {
            assert!(a.allow(std::net::Ipv4Addr::from(n).into(), now));
        }
        assert!(!a.allow(std::net::Ipv4Addr::from(1025).into(), now));
        assert_eq!(a.entries.len(), 1024);
    }
}
