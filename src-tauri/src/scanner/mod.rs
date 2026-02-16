pub mod projects;
pub mod sessions;
pub mod subagents;

use std::sync::OnceLock;

/// Dedicated rayon thread pool for session/project scanning.
/// Capped at 8 threads to avoid overwhelming I/O on large project directories.
static SCAN_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

pub fn scan_pool() -> &'static rayon::ThreadPool {
    SCAN_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .thread_name(|i| format!("scan-pool-{i}"))
            .build()
            .expect("Failed to build scan thread pool")
    })
}
