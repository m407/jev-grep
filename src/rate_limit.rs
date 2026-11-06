use anyhow::{Result, ensure};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const WINDOW: Duration = Duration::from_secs(60);

// No model-specific tokenizer is configured for System One. Conservatively count
// one token per UTF-8 byte of the entire serialized JSON, plus 256 tokens for
// server-side framing/output. This includes all questions and accumulated state;
// it is an estimate, not provider billing or a bound on unknown server prompts.
pub fn estimate_request_tokens(body: &[u8], safety_factor: f64) -> Result<u64> {
    let estimate = ((body.len() as f64 + 256.0) * safety_factor).ceil();
    ensure!(
        safety_factor.is_finite() && safety_factor >= 1.0,
        "tpm-safety-factor must be finite and at least 1"
    );
    ensure!(
        estimate.is_finite() && estimate < u64::MAX as f64,
        "request token estimate exceeds the supported range; reduce tpm-safety-factor or request size"
    );
    Ok(estimate as u64)
}

pub struct RateLimiter {
    pub limit: Option<u64>,
    pub used: u64,
    reservations: VecDeque<(Instant, u64)>,
}

impl RateLimiter {
    pub fn new(limit: Option<u64>) -> Self {
        Self {
            limit,
            used: 0,
            reservations: VecDeque::new(),
        }
    }

    pub fn expire(&mut self, now: Instant) {
        while self
            .reservations
            .front()
            .is_some_and(|(sent, _)| now.duration_since(*sent) >= WINDOW)
        {
            self.used -= self.reservations.pop_front().unwrap().1;
        }
    }

    /// Reserve now, or return the time until enough budget expires. Reservations
    /// are never refunded, including when transport fails after reservation.
    pub fn reserve(&mut self, cost: u64, now: Instant) -> Result<Option<Duration>> {
        let Some(limit) = self.limit else {
            return Ok(None);
        };
        ensure!(
            cost <= limit,
            "request estimate {cost} tokens exceeds local TPM budget {limit}; increase --tpm or reduce the query/fragment size"
        );
        self.expire(now);
        if cost <= limit - self.used {
            self.reservations.push_back((now, cost));
            self.used += cost;
            return Ok(None);
        }
        let mut remaining = self.used;
        for &(sent, tokens) in &self.reservations {
            remaining -= tokens;
            if cost <= limit - remaining {
                return Ok(Some(WINDOW - now.duration_since(sent)));
            }
        }
        unreachable!("a request fitting the budget must fit after all reservations expire")
    }
}

pub trait Clock {
    fn now(&self) -> Instant;
    fn sleep(&self, delay: Duration);
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, delay: Duration) {
        std::thread::sleep(delay);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sliding_window_waits_for_enough_expirations_and_reserves_at_send_time() {
        let now = Instant::now();
        let mut limiter = RateLimiter::new(Some(100));
        assert_eq!(limiter.reserve(40, now).unwrap(), None);
        assert_eq!(
            limiter.reserve(60, now + Duration::from_secs(10)).unwrap(),
            None
        );
        assert_eq!(
            limiter.reserve(70, now + Duration::from_secs(20)).unwrap(),
            Some(Duration::from_secs(50))
        );
        // Crossing a wall-clock minute does not clear the second reservation.
        assert_eq!(
            limiter.reserve(70, now + WINDOW).unwrap(),
            Some(Duration::from_secs(10))
        );
        assert_eq!(limiter.used, 60);
        assert_eq!(
            limiter.reserve(70, now + Duration::from_secs(70)).unwrap(),
            None
        );
        assert_eq!(limiter.used, 70);
        assert_eq!(
            limiter.reserve(31, now + Duration::from_secs(70)).unwrap(),
            Some(WINDOW)
        );
    }

    #[test]
    fn disabled_oversized_and_exact_budget_requests() {
        let now = Instant::now();
        let mut disabled = RateLimiter::new(None);
        for _ in 0..4 {
            assert_eq!(disabled.reserve(u64::MAX, now).unwrap(), None);
        }
        assert_eq!(disabled.used, 0);
        let mut limiter = RateLimiter::new(Some(u64::MAX));
        assert_eq!(limiter.reserve(u64::MAX, now).unwrap(), None);
        assert_eq!(limiter.reserve(1, now).unwrap(), Some(WINDOW));
        let mut limiter = RateLimiter::new(Some(10));
        assert!(
            limiter
                .reserve(11, now)
                .unwrap_err()
                .to_string()
                .contains("exceeds local TPM budget")
        );
        assert_eq!(limiter.used, 0);
    }

    #[test]
    fn estimation_counts_utf8_json_instructions_and_rounds_up() {
        let body = br#"{"model":"jev","state":"a","questions":{"match":{"instructions":"b"}}}"#;
        assert_eq!(
            estimate_request_tokens(body, 1.2).unwrap(),
            ((body.len() + 256) as f64 * 1.2).ceil() as u64
        );
        assert_eq!(
            estimate_request_tokens("ошибка".as_bytes(), 1.0).unwrap(),
            268
        );
        for factor in [0.0, 0.9, f64::NAN, f64::INFINITY, f64::MAX] {
            assert!(estimate_request_tokens(body, factor).is_err());
        }
    }
}
