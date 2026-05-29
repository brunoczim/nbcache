//! Exhaustive concurrency models for the per-slot seqlock in `nbcache::raw`.
//!
//! These run only under `--cfg loom` (see `cargo xtask loom`); loom replaces
//! the cache's atomics with instrumented ones and explores every legal thread
//! interleaving, which is the only way to deterministically probe the
//! CAS-retry / `outdated` paths and to confirm or refute torn reads.
#![cfg(loom)]

use std::hash::{BuildHasher, Hasher};

use loom::{sync::Arc, thread};
use nbcache::raw::NbCacheWith;

/// Hashes everything to slot 0, so all keys contend on a single `Entry`.
#[derive(Clone, Default)]
struct ZeroBuildHasher;

impl BuildHasher for ZeroBuildHasher {
    type Hasher = ZeroHasher;

    fn build_hasher(&self) -> ZeroHasher {
        ZeroHasher
    }
}

struct ZeroHasher;

impl Hasher for ZeroHasher {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, _bytes: &[u8]) {}
}

/// The value stored alongside each key. A torn read pairs one writer's key
/// word with another writer's value word, so `value != value_of(key)`.
fn value_of(key: u32) -> u32 {
    key.wrapping_mul(0x9E37_79B9).wrapping_add(1)
}

fn cache() -> NbCacheWith<ZeroBuildHasher, 1, 1> {
    NbCacheWith::with_hasher(1, ZeroBuildHasher)
}

/// One writer, one concurrent reader: a read must never observe a key paired
/// with the wrong value, and the slot must settle on the written pair.
#[test]
fn single_writer_one_reader_is_consistent() {
    loom::model(|| {
        let cache = Arc::new(cache());

        let writer = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || cache.put([1], [value_of(1)]))
        };

        if let Some([v]) = cache.get([1]) {
            assert_eq!(v, value_of(1), "torn read with a single writer");
        }

        writer.join().unwrap();
        assert_eq!(cache.get([1]), Some([value_of(1)]));
    });
}

/// Two writers race on the same slot while a reader observes. The reader may
/// see either key, the empty sentinel, or nothing, but never a key carrying a
/// foreign value.
///
/// Ignored: the current `write_pair` livelocks under concurrent writers (see
/// its comment in `raw.rs`), so some interleavings never terminate and would
/// hang loom rather than fail an assertion. Re-enable once the write path is
/// fixed — loom should then either prove the torn read or confirm the fix.
#[test]
#[ignore = "concurrent-writer livelock in write_pair would hang loom; re-enable after fix"]
fn two_writers_one_reader_never_tear() {
    loom::model(|| {
        let cache = Arc::new(cache());

        let t1 = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || cache.put([1], [value_of(1)]))
        };
        let t2 = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || cache.put([2], [value_of(2)]))
        };

        if let Some([v]) = cache.get([1]) {
            assert_eq!(v, value_of(1), "torn read: key 1 with a foreign value");
        }
        if let Some([v]) = cache.get([2]) {
            assert_eq!(v, value_of(2), "torn read: key 2 with a foreign value");
        }

        t1.join().unwrap();
        t2.join().unwrap();
    });
}
