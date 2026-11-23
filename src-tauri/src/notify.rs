use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use uuid::Uuid;

const MAX_PER_MINUTE: usize = 10;
const WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub tag: Option<String>,
    pub path: Option<String>,
}

pub fn request_identifier(server: Uuid, tag: Option<&str>) -> String {
    match tag {
        Some(tag) => format!("{server}:{tag}"),
        None => format!("{server}:{}", Uuid::new_v4()),
    }
}

#[derive(Default)]
pub struct Limiter {
    admitted: VecDeque<Instant>,
}

impl Limiter {
    pub fn allow(&mut self, now: Instant) -> bool {
        while self
            .admitted
            .front()
            .is_some_and(|oldest| now.saturating_duration_since(*oldest) >= WINDOW)
        {
            self.admitted.pop_front();
        }
        if self.admitted.len() >= MAX_PER_MINUTE {
            return false;
        }
        self.admitted.push_back(now);
        true
    }
}

#[derive(Default)]
pub struct NotifyLimits(Mutex<HashMap<Uuid, Limiter>>);

impl NotifyLimits {
    fn limiters(&self) -> MutexGuard<'_, HashMap<Uuid, Limiter>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn allow(&self, server: Uuid, now: Instant) -> bool {
        self.limiters().entry(server).or_default().allow(now)
    }

    pub fn forget(&self, server: Uuid) {
        self.limiters().remove(&server);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_makes_the_identifier_stable_and_no_tag_makes_it_fresh() {
        let server = Uuid::new_v4();

        assert_eq!(
            request_identifier(server, Some("task-1")),
            format!("{server}:task-1")
        );

        let first = request_identifier(server, None);
        let second = request_identifier(server, None);
        assert!(first.starts_with(&format!("{server}:")));
        assert!(second.starts_with(&format!("{server}:")));
        assert_ne!(first, second);
    }

    #[test]
    fn admits_ten_in_a_minute_and_refuses_the_eleventh() {
        let start = Instant::now();
        let mut limiter = Limiter::default();

        for second in 0..10 {
            assert!(
                limiter.allow(start + Duration::from_secs(second)),
                "{second}"
            );
        }
        assert!(!limiter.allow(start + Duration::from_secs(59)));
    }

    #[test]
    fn admits_again_once_the_oldest_is_a_minute_old() {
        let start = Instant::now();
        let mut limiter = Limiter::default();
        for _ in 0..10 {
            assert!(limiter.allow(start));
        }

        assert!(!limiter.allow(start + Duration::from_secs(59)));
        assert!(limiter.allow(start + Duration::from_secs(60)));
    }

    #[test]
    fn keeps_two_servers_apart_and_forgets_one() {
        let limits = NotifyLimits::default();
        let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
        let now = Instant::now();
        for _ in 0..10 {
            assert!(limits.allow(first, now));
        }

        assert!(!limits.allow(first, now));
        assert!(limits.allow(second, now));

        limits.forget(first);
        assert!(limits.allow(first, now));
    }
}
