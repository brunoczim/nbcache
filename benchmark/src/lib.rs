use std::{
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

use nbcache::Cache;
use rand::{
    distr::{Distribution, StandardUniform},
    random,
    random_iter,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchmarkConfig {
    pub thread_count: usize,
    pub key_count: usize,
    pub get_interval: usize,
    pub put_interval: usize,
    pub delete_interval: usize,
}

pub fn run_once<C>(config: BenchmarkConfig, cache: C) -> Duration
where
    C: Cache + Clone + Send + Sync + 'static,
    C::Key: Copy + Send + Sync + 'static,
    C::Value: Send + Sync + 'static,
    StandardUniform: Distribution<C::Key>,
    StandardUniform: Distribution<C::Value>,
{
    let mut threads = Vec::new();
    let keys: Arc<[C::Key]> = random_iter().take(config.key_count).collect();
    let barrier = Arc::new(Barrier::new(config.thread_count + 1));

    for i in 0 .. config.thread_count {
        let cache = cache.clone();
        let barrier = barrier.clone();
        let keys = keys.clone();
        threads.push(thread::spawn(move || {
            barrier.wait();
            for (j, key) in keys.iter().copied().enumerate() {
                if (j + i)
                    .checked_rem(config.put_interval)
                    .is_some_and(|rem| rem == 0)
                {
                    cache.put(key, random());
                }
                if (j + i + 1)
                    .checked_rem(config.get_interval)
                    .is_some_and(|rem| rem == 0)
                {
                    cache.get(key);
                }
                if (j + i + 2)
                    .checked_rem(config.delete_interval)
                    .is_some_and(|rem| rem == 0)
                {
                    cache.delete(key);
                }
            }
        }));
    }

    barrier.wait();
    let then = Instant::now();
    for thread in threads {
        thread.join().unwrap();
    }
    then.elapsed()
}
