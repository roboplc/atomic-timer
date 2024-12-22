<h2>
  atomic-timer - an atomic timer for Rust
  <a href="https://crates.io/crates/atomic-timer"><img alt="crates.io page" src="https://img.shields.io/crates/v/atomic-timer.svg"></img></a>
  <a href="https://docs.rs/atomic-timer"><img alt="docs.rs page" src="https://docs.rs/atomic-timer/badge.svg"></img></a>
  <a href="https://github.com/roboplc/atomic-timer/actions/workflows/ci.yml">
    <img alt="GitHub Actions CI" src="https://github.com/roboplc/atomic-timer/actions/workflows/ci.yml/badge.svg"></img>
  </a>
</h2>

A passive timer object which can be manipulated atomically. Useful for
automation and robotics tasks.

Atomic timer is a part of [RoboPLC](https://www.roboplc.com) project.

## Usage example

### Basic usage

```rust
use atomic_timer::AtomicTimer;
use std::time::Duration;

let timer = AtomicTimer::new(Duration::from_secs(1));
for _ in 0..100 {
    if timer.expired() {
        println!("Timer expired");
        timer.reset(); // does not need to be mutable
    } else {
        println!("Elapsed: {:?}, remaining: {:?}", timer.elapsed(), timer.remaining());
    }
    // do some work
}
```

### Multi-threaded usage

```rust
use atomic_timer::AtomicTimer;
use std::sync::Arc;
use std::time::Duration;

let timer = Arc::new(AtomicTimer::new(Duration::from_secs(1)));
for _ in 0..10 {
    let timer = timer.clone();
    std::thread::spawn(move || {
        for _ in 0..100 {
            if timer.reset_if_expired() {
                println!("Timer expired");
                // react to the timer expiration
                // guaranteed to be true only for one thread
            }
            // do some other work
        }
    });
}
```

## Serialization / deserialization

Atomic timer objects can be safely serialized and de-serialized (requires
`serde` feature).

When a timer is de-serialized, it keeps its state (elapsed/remaining time),
despite the system monotonic clock difference.

## Limitations

Currently does not works on WASM (can be added, write an issue if you really need it).

## MSRV

1.68.0
