#![ doc = include_str!( concat!( env!( "CARGO_MANIFEST_DIR" ), "/", "README.md" ) ) ]
#![deny(missing_docs)]
use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::Duration,
};

use bma_ts::Monotonic;

/// Atomic timer
pub struct AtomicTimer {
    duration: i64,
    start: AtomicI64,
    monotonic_fn: fn() -> i64,
}

fn monotonic_ns() -> i64 {
    i64::try_from(Monotonic::now().as_nanos()).expect("Monotonic time is too large")
}

impl AtomicTimer {
    #[allow(dead_code)]
    fn construct(duration: i64, elapsed: i64, monotonic_fn: fn() -> i64) -> Self {
        AtomicTimer {
            duration,
            start: AtomicI64::new(monotonic_fn() - elapsed),
            monotonic_fn,
        }
    }
    /// Create a new atomic timer
    ///
    /// # Panics
    ///
    /// Panics if the duration is too large (in nanos greater than `i64::MAX`)
    pub fn new(duration: Duration) -> Self {
        AtomicTimer {
            duration: duration
                .as_nanos()
                .try_into()
                .expect("Duration is too large"),
            start: AtomicI64::new(monotonic_ns()),
            monotonic_fn: monotonic_ns,
        }
    }
    /// Reset the timer
    #[inline]
    pub fn reset(&self) {
        self.start.store((self.monotonic_fn)(), Ordering::SeqCst);
    }
    /// Focibly expire the timer
    pub fn expire_now(&self) {
        self.start
            .store((self.monotonic_fn)() - self.duration, Ordering::SeqCst);
    }
    /// Reset the timer if it has expired, return true if reset
    #[inline]
    pub fn reset_if_expired(&self) -> bool {
        let now = (self.monotonic_fn)();
        self.start
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |start| {
                (now.saturating_sub(start) >= self.duration).then_some(now)
            })
            .is_ok()
    }
    /// Get the elapsed time
    #[inline]
    pub fn elapsed(&self) -> Duration {
        Duration::from_nanos(
            (self.monotonic_fn)()
                .saturating_sub(self.start.load(Ordering::SeqCst))
                .try_into()
                .unwrap_or_default(),
        )
    }
    /// Get the remaining time
    #[inline]
    pub fn remaining(&self) -> Duration {
        let elapsed = self.elapsed_ns();
        if elapsed >= self.duration {
            Duration::ZERO
        } else {
            Duration::from_nanos((self.duration - elapsed).try_into().unwrap_or_default())
        }
    }
    fn elapsed_ns(&self) -> i64 {
        (self.monotonic_fn)().saturating_sub(self.start.load(Ordering::SeqCst))
    }
    /// Check if the timer has expired
    #[inline]
    pub fn expired(&self) -> bool {
        self.elapsed_ns() >= self.duration
    }
}

#[cfg(feature = "serde")]
mod ser {
    use super::{monotonic_ns, AtomicTimer};
    use serde::{ser::SerializeTuple as _, Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for AtomicTimer {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            // serialize duration and elapsed into a tuple
            let mut t = serializer.serialize_tuple(2)?;
            t.serialize_element(&self.duration)?;
            t.serialize_element(&self.elapsed_ns())?;
            t.end()
        }
    }

    impl<'de> Deserialize<'de> for AtomicTimer {
        fn deserialize<D>(deserializer: D) -> Result<AtomicTimer, D::Error>
        where
            D: Deserializer<'de>,
        {
            // deserialize duration and elapsed from a tuple
            let (duration, elapsed) = <(i64, i64)>::deserialize(deserializer)?;
            Ok(AtomicTimer::construct(duration, elapsed, monotonic_ns))
        }
    }
}

#[cfg(test)]
mod test {
    use super::AtomicTimer;
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::Duration,
    };

    pub(crate) fn in_time_window(a: Duration, b: Duration, window: Duration) -> bool {
        let diff = window / 2;
        let min = b - diff;
        let max = b + diff;
        a >= min && a <= max
    }

    #[test]
    fn test_reset() {
        let timer = AtomicTimer::new(Duration::from_secs(5));
        thread::sleep(Duration::from_secs(1));
        timer.reset();
        assert!(timer.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn test_expire_now() {
        let timer = AtomicTimer::new(Duration::from_secs(5));
        assert!(!timer.expired());
        assert!(in_time_window(
            timer.remaining(),
            Duration::from_secs(5),
            Duration::from_millis(100)
        ));
        timer.expire_now();
        assert!(timer.expired());
    }

    #[test]
    fn test_reset_if_expired() {
        let timer = AtomicTimer::new(Duration::from_secs(1));
        assert!(!timer.reset_if_expired());
        thread::sleep(Duration::from_millis(1100));
        assert!(timer.expired());
        assert!(timer.reset_if_expired());
    }

    #[test]
    fn test_reset_if_expired_no_datarace() {
        let n = 1000;
        let timer = Arc::new(AtomicTimer::new(Duration::from_millis(100)));
        thread::sleep(Duration::from_millis(200));
        assert!(timer.expired());
        let barrier = Arc::new(Barrier::new(n));
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        let mut result = Vec::with_capacity(n);
        for _ in 0..n {
            let timer = timer.clone();
            let barrier = barrier.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                barrier.wait();
                tx.send(timer.reset_if_expired()).unwrap();
            });
        }
        drop(tx);
        while let Ok(v) = rx.recv() {
            result.push(v);
        }
        assert_eq!(result.len(), n);
        assert_eq!(result.into_iter().filter(|&v| v).count(), 1);
    }
}

#[cfg(feature = "serde")]
#[cfg(test)]
mod test_serialization {
    use super::test::in_time_window;
    use super::AtomicTimer;
    use std::{thread, time::Duration};

    #[test]
    fn test_serialize_deserialize() {
        let timer = AtomicTimer::new(Duration::from_secs(5));
        thread::sleep(Duration::from_secs(1));
        let serialized = serde_json::to_string(&timer).unwrap();
        let deserialized: AtomicTimer = serde_json::from_str(&serialized).unwrap();
        assert!(in_time_window(
            deserialized.elapsed(),
            Duration::from_secs(1),
            Duration::from_millis(100)
        ));
    }

    #[test]
    fn test_serialize_deserialize_monotonic_goes_forward() {
        fn monotonic_ns_forwarded() -> i64 {
            super::monotonic_ns() + 10_000 * 1_000_000_000
        }
        let timer = AtomicTimer::new(Duration::from_secs(5));
        thread::sleep(Duration::from_secs(1));
        let serialized = serde_json::to_string(&timer).unwrap();
        let deserialized: AtomicTimer = serde_json::from_str(&serialized).unwrap();
        let deserialized_rewinded = AtomicTimer::construct(
            deserialized.duration,
            deserialized.elapsed_ns(),
            monotonic_ns_forwarded,
        );
        assert!(in_time_window(
            deserialized_rewinded.elapsed(),
            Duration::from_secs(1),
            Duration::from_millis(100)
        ));
    }

    #[test]
    fn test_serialize_deserialize_monotonic_goes_backward() {
        fn monotonic_ns_forwarded() -> i64 {
            super::monotonic_ns() - 10_000 * 1_000_000_000
        }
        let timer = AtomicTimer::new(Duration::from_secs(5));
        thread::sleep(Duration::from_secs(1));
        let serialized = serde_json::to_string(&timer).unwrap();
        let deserialized: AtomicTimer = serde_json::from_str(&serialized).unwrap();
        let deserialized_rewinded = AtomicTimer::construct(
            deserialized.duration,
            deserialized.elapsed_ns(),
            monotonic_ns_forwarded,
        );
        assert!(in_time_window(
            deserialized_rewinded.elapsed(),
            Duration::from_secs(1),
            Duration::from_millis(100)
        ));
    }

    #[test]
    fn test_serialize_deserialize_monotonic_goes_zero() {
        fn monotonic_ns_forwarded() -> i64 {
            0
        }
        let timer = AtomicTimer::new(Duration::from_secs(5));
        thread::sleep(Duration::from_secs(1));
        let serialized = serde_json::to_string(&timer).unwrap();
        let deserialized: AtomicTimer = serde_json::from_str(&serialized).unwrap();
        let deserialized_rewinded = AtomicTimer::construct(
            deserialized.duration,
            deserialized.elapsed_ns(),
            monotonic_ns_forwarded,
        );
        assert!(in_time_window(
            deserialized_rewinded.elapsed(),
            Duration::from_secs(1),
            Duration::from_millis(100)
        ));
    }
}
