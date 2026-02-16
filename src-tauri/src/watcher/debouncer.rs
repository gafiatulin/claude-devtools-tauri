use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::EventKind;

/// A debounced file event preserving the original notify EventKind.
#[derive(Debug, Clone)]
pub struct DebouncedFileEvent {
    pub path: PathBuf,
    pub kind: EventKind,
}

/// Entry tracking the latest event for a given path.
struct PendingEvent {
    kind: EventKind,
    last_seen: Instant,
}

/// Debounces rapid file system events per-path, preserving `notify::EventKind`.
///
/// Events for the same path within the debounce window are coalesced.
/// The last event kind seen wins (e.g. Create then Modify within the
/// window emits a single Modify).
///
/// Uses a channel-based design: the timer thread blocks on recv_timeout
/// instead of a sleep loop, so it only wakes when events arrive or when
/// the timeout expires with pending work.
pub struct Debouncer {
    tx: mpsc::Sender<(PathBuf, EventKind)>,
    timeout: Duration,
    _timer_handle: std::thread::JoinHandle<()>,
}

impl Debouncer {
    /// Create a new debouncer that fires `callback` with batches of coalesced
    /// events. The callback is invoked at most once per `timeout` interval.
    pub fn new<F>(timeout: Duration, callback: F) -> Self
    where
        F: Fn(Vec<DebouncedFileEvent>) + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<(PathBuf, EventKind)>();

        let handle = std::thread::spawn(move || {
            let mut pending: HashMap<PathBuf, PendingEvent> = HashMap::new();

            loop {
                // Block until an event arrives or timeout expires
                let wait_result = if pending.is_empty() {
                    // No pending events: block indefinitely until one arrives
                    match rx.recv() {
                        Ok(event) => Ok(event),
                        Err(_) => break, // Channel closed
                    }
                } else {
                    // Pending events: wait up to timeout for more
                    match rx.recv_timeout(timeout) {
                        Ok(event) => Ok(event),
                        Err(mpsc::RecvTimeoutError::Timeout) => Err(()),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                };

                // Accumulate the received event (if any)
                if let Ok((path, kind)) = wait_result {
                    let now = Instant::now();
                    let entry = pending.entry(path).or_insert_with(|| PendingEvent {
                        kind,
                        last_seen: now,
                    });
                    entry.kind = kind;
                    entry.last_seen = now;

                    // Drain any other events already in the channel
                    while let Ok((path, kind)) = rx.try_recv() {
                        let now = Instant::now();
                        let entry = pending.entry(path).or_insert_with(|| PendingEvent {
                            kind,
                            last_seen: now,
                        });
                        entry.kind = kind;
                        entry.last_seen = now;
                    }

                    // Don't flush yet — wait for the timeout
                    continue;
                }

                // Timeout expired — flush events that have been quiet long enough
                let now = Instant::now();
                let mut batch = Vec::new();
                pending.retain(|path, entry| {
                    if now.duration_since(entry.last_seen) >= timeout {
                        batch.push(DebouncedFileEvent {
                            path: path.clone(),
                            kind: entry.kind,
                        });
                        false // remove from pending
                    } else {
                        true // keep — still within debounce window
                    }
                });

                if !batch.is_empty() {
                    callback(batch);
                }
            }
        });

        Self {
            tx,
            timeout,
            _timer_handle: handle,
        }
    }

    /// Record a raw notify event. It will be coalesced and emitted after
    /// the debounce timeout elapses with no further events for the same path.
    pub fn add_event(&self, path: PathBuf, kind: EventKind) {
        let _ = self.tx.send((path, kind));
    }

    /// Returns the debounce timeout for this instance.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}
