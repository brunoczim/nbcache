use std::hash::{BuildHasher, RandomState};

// Atomics are sourced from `loom` under `--cfg loom` so the concurrency models
// in `tests/loom.rs` can explore interleavings exhaustively; every other build
// uses the real `std` atomics. The two types are API-compatible for our use.
#[cfg(loom)]
use loom::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(loom))]
use std::sync::atomic::{AtomicU64, Ordering};

pub type Key<const K: usize> = [u32; K];
pub type Value<const V: usize> = [u32; V];

pub type NbCache<const K: usize, const V: usize> = NbCacheWith<RandomState, K, V>;

#[derive(Debug)]
pub struct NbCacheWith<H, const K: usize, const V: usize> {
    entries: Box<[Entry<K, V>]>,
    build_hasher: H,
    empty_key: Key<K>,
}

impl<const K: usize, const V: usize> NbCache<K, V> {
    pub fn new(size: usize) -> Self {
        Self::with_hasher(size, RandomState::new())
    }

    pub fn with_empty(size: usize, empty_key: Key<K>) -> Self {
        Self::with_hasher_and_empty(size, RandomState::new(), empty_key)
    }
}

impl<H, const K: usize, const V: usize> NbCacheWith<H, K, V>
where
    H: BuildHasher,
{
    pub fn with_hasher(size: usize, build_hasher: H) -> Self {
        Self::with_hasher_and_empty(size, build_hasher, [0; K])
    }

    pub fn with_hasher_and_empty(size: usize, build_hasher: H, empty_key: Key<K>) -> Self {
        assert_ne!(size, 0);

        let mut entries = Vec::with_capacity(size);
        for _ in 0..size {
            entries.push(Entry::new(empty_key));
        }
        Self {
            entries: entries.into(),
            build_hasher,
            empty_key,
        }
    }

    pub fn get(&self, key: Key<K>) -> Option<Value<V>> {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let (stored_key, stored_value) = self.entries[index].read_pair();
        if stored_key == key {
            Some(stored_value)
        } else {
            None
        }
    }

    pub fn put(&self, key: Key<K>, value: Value<V>) {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        self.entries[index].write_pair(key, value);
    }

    pub fn delete(&self, key: Key<K>) -> bool {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        self.entries[index].write_key_if_equal(key, self.empty_key)
    }
}

#[derive(Debug)]
struct Entry<const K: usize, const V: usize> {
    version: AtomicU64,
    alternates: [EntryAlternate<K, V>; 2],
}

impl<const K: usize, const V: usize> Entry<K, V> {
    pub fn new(key: Key<K>) -> Self {
        Self {
            version: AtomicU64::new(0),
            alternates: [
                EntryAlternate::new(key, [0; V], 0),
                EntryAlternate::new(key, [0; V], u32::MAX),
            ],
        }
    }

    pub fn read_pair(&self) -> (Key<K>, Value<V>) {
        let mut curr_version = self.version.load(Ordering::Acquire);

        'main: loop {
            let alternate_index = (curr_version & 1) as usize;
            let tag = curr_version >> 1;
            let alternate = &self.alternates[alternate_index];

            let mut key = [0; K];
            let mut value = [0; V];

            for (dest, src) in key
                .iter_mut()
                .chain(&mut value)
                .zip(alternate.key.iter().chain(&alternate.value))
            {
                let data = src.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let tag_low = tag & 0xff_ff_ff_ff;
                if embedded_tag != tag_low {
                    curr_version = self.version.load(Ordering::Acquire);
                    continue 'main;
                }
                *dest = data as u32;
            }

            break (key, value);
        }
    }

    pub fn read_versioned_key(&self) -> (Key<K>, u64) {
        let mut curr_version = self.version.load(Ordering::Acquire);

        'main: loop {
            let alternate_index = (curr_version & 1) as usize;
            let tag = curr_version >> 1;
            let alternate = &self.alternates[alternate_index];

            let mut key = [0; K];

            for (dest, src) in key.iter_mut().zip(alternate.key.iter()) {
                let data = src.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let tag_low = tag & 0xff_ff_ff_ff;
                if embedded_tag != tag_low {
                    curr_version = self.version.load(Ordering::Acquire);
                    continue 'main;
                }
                *dest = data as u32;
            }

            break (key, curr_version);
        }
    }

    // KNOWN BUG — concurrent writers to the same slot are unsafe, in two ways:
    //
    // 1. Livelock. Every writer that observes `curr_version` targets the same
    //    non-current alternate and stamps each word with the same `next_tag`.
    //    A writer keeps CAS-ing words even after it has set `outdated`, so two
    //    interleaved writers can leave that alternate holding a mix of
    //    `next_tag` words that no longer matches `prev_tag`. From then on every
    //    writer sees `outdated` on every pass and none can take the `version`
    //    CAS, so the slot's version is wedged forever and `put` never returns.
    //    (Reproduced by the ignored test in `tests/livelock.rs`.)
    //
    // 2. Torn reads. Because racing writers share `next_tag`, one writer can
    //    complete a clean pass and publish the new `version` while another has
    //    already overwritten a word it CAS'd, so the slot exposes key/value
    //    words from different writers — a pair that was never inserted, yet
    //    whose per-word tags all match the current tag and so passes the reader
    //    check in `read_pair`.
    //
    // Left as-is pending review; single-writer use with concurrent readers is
    // fine.
    //
    // RECOMMENDED FIX (not yet applied): reserve the alternate via the version
    // word so writers are mutually exclusive per slot while reads stay
    // non-blocking. Add a write-lock bit to `version`:
    //
    //     version = (tag << 2) | (write_lock << 1) | current_index
    //
    // Readers are unchanged in spirit: `current = version & 1`,
    // `tag = version >> 2`, and the lock bit is ignored (a writer never touches
    // the current alternate, so reads stay consistent).
    //
    // Writer:
    //   1. Load `version`; if the lock bit is set, retry (a writer is active).
    //   2. CAS `version` -> `version | LOCK` to acquire exclusive write access;
    //      retry on failure.
    //   3. Own the non-current alternate exclusively: store every word with
    //      `next_tag` via plain `Release` stores. No per-word CAS and no
    //      `outdated` check are needed, because no other writer can touch it.
    //   4. Publish with a single `Release` store of
    //      `version = (next_tag << 2) | next_alt_index` (lock cleared, current
    //      flipped). No CAS is needed: while we hold the lock no other writer
    //      changes `version`, and readers never write it.
    //
    // `write_key_if_equal` uses the same acquire step, then reads the current
    // key directly (stable under the lock) to compare against `expected`.
    //
    // This removes the `outdated` flag and the in-loop CAS retries entirely (a
    // net simplification). Trade-off: writers serialize per slot rather than
    // racing lock-free, while reads remain non-blocking, which matches this
    // crate's intent. Alternatives: (a) document a single-writer-per-slot
    // contract and keep this code as-is; (b) a true lock-free multi-writer
    // scheme, which is substantially more complex and likely overkill here.
    pub fn write_pair(&self, key: Key<K>, value: Value<V>) {
        let mut curr_version = self.version.load(Ordering::Acquire);

        'main: loop {
            let alternate_index = (curr_version & 1) as usize;
            let next_alt_index = 1 - alternate_index;
            let tag = curr_version >> 1;
            let alternate = &self.alternates[next_alt_index];
            let prev_tag = tag.wrapping_sub(1);
            let next_tag = tag.wrapping_add(1);

            let mut outdated = false;
            for (dest, src) in alternate
                .key
                .iter()
                .chain(&alternate.value)
                .zip(key.into_iter().chain(value))
            {
                let data = dest.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let prev_tag_low = prev_tag & 0xff_ff_ff_ff;
                if embedded_tag != prev_tag_low {
                    outdated = true;
                }
                let new_data = u64::from(src) | (next_tag << 32);
                if dest
                    .compare_exchange(data, new_data, Ordering::Release, Ordering::Relaxed)
                    .is_err()
                {
                    curr_version = self.version.load(Ordering::Acquire);
                    continue 'main;
                }
            }

            if outdated {
                curr_version = self.version.load(Ordering::Acquire);
                continue;
            }

            let next_version = (next_tag << 1) | next_alt_index as u64;
            match self.version.compare_exchange(
                curr_version,
                next_version,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(actual) => curr_version = actual,
            }
        }
    }

    // Shares `write_pair`'s concurrent-writer hazards (livelock and torn reads);
    // see the comment there. Safe with a single writer plus concurrent readers.
    pub fn write_key_if_equal(&self, expected: Key<K>, new: Key<K>) -> bool {
        'main: loop {
            let (curr_key, curr_version) = self.read_versioned_key();
            if curr_key != expected {
                return false;
            }

            let alternate_index = (curr_version & 1) as usize;
            let next_alt_index = 1 - alternate_index;
            let tag = curr_version >> 1;
            let alternate = &self.alternates[next_alt_index];
            let prev_tag = tag.wrapping_sub(1);
            let next_tag = tag.wrapping_add(1);

            let mut outdated = false;
            for (dest, src) in alternate.key.iter().zip(new) {
                let data = dest.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let prev_tag_low = prev_tag & 0xff_ff_ff_ff;
                if embedded_tag != prev_tag_low {
                    outdated = true;
                }
                let new_data = u64::from(src) | (next_tag << 32);
                if dest
                    .compare_exchange(data, new_data, Ordering::Release, Ordering::Relaxed)
                    .is_err()
                {
                    continue 'main;
                }
            }

            for dest in alternate.value.iter() {
                let data = dest.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let prev_tag_low = prev_tag & 0xff_ff_ff_ff;
                if embedded_tag != prev_tag_low {
                    outdated = true;
                }
                let new_data = next_tag << 32;
                if dest
                    .compare_exchange(data, new_data, Ordering::Release, Ordering::Relaxed)
                    .is_err()
                {
                    continue 'main;
                }
            }

            if outdated {
                continue;
            }

            let next_version = (next_tag << 1) | next_alt_index as u64;
            if self
                .version
                .compare_exchange(
                    curr_version,
                    next_version,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return true;
            }
        }
    }
}

#[derive(Debug)]
struct EntryAlternate<const K: usize, const V: usize> {
    key: [AtomicU64; K],
    value: [AtomicU64; V],
}

impl<const K: usize, const V: usize> EntryAlternate<K, V> {
    pub fn new(key: Key<K>, value: Value<V>, tag: u32) -> Self {
        Self {
            key: key
                .map(u64::from)
                .map(|bits| bits | ((tag as u64) << 32))
                .map(AtomicU64::new),
            value: value
                .map(u64::from)
                .map(|bits| bits | ((tag as u64) << 32))
                .map(AtomicU64::new),
        }
    }
}

// Inline tests are skipped under `--cfg loom`: they rely on `std` threads and
// the loom build replaces the atomics with instrumented ones. The exhaustive
// concurrency models live in `tests/loom.rs`.
#[cfg(all(test, not(loom)))]
mod tests {
    use std::hash::Hasher;

    use super::*;

    /// Deterministic `BuildHasher` whose `hash_one([a, ..])` is `a`, i.e. the
    /// first key word. This makes slot selection (`hash % size`) predictable,
    /// so tests can force collisions or spread keys across slots on purpose,
    /// unlike the randomized `RandomState` used by `NbCache::new`.
    #[derive(Clone, Default)]
    struct FirstWordBuildHasher;

    impl BuildHasher for FirstWordBuildHasher {
        type Hasher = FirstWordHasher;

        fn build_hasher(&self) -> FirstWordHasher {
            FirstWordHasher::default()
        }
    }

    #[derive(Default)]
    struct FirstWordHasher {
        value: u64,
        written: bool,
    }

    impl Hasher for FirstWordHasher {
        fn finish(&self) -> u64 {
            self.value
        }

        // `[u32; K]` hashes as a slice: a `write_usize` length prefix followed
        // by a single `write` of all element bytes in native-endian order. We
        // drop the length prefix and read the first word's bytes, so
        // `hash_one([a, ..]) == a` and the slot is simply `a % size`.
        fn write_usize(&mut self, _len: usize) {}

        fn write(&mut self, bytes: &[u8]) {
            if !self.written && bytes.len() >= 4 {
                self.value =
                    u64::from(u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
                self.written = true;
            }
        }
    }

    /// A cache with the deterministic hasher and the default empty key `[0; K]`.
    fn det<const K: usize, const V: usize>(size: usize) -> NbCacheWith<FirstWordBuildHasher, K, V> {
        NbCacheWith::with_hasher(size, FirstWordBuildHasher)
    }

    #[test]
    fn put_then_get_roundtrips() {
        let cache = NbCache::<1, 1>::new(16);
        cache.put([42], [7]);
        assert_eq!(cache.get([42]), Some([7]));
    }

    #[test]
    fn get_on_absent_key_is_none() {
        let cache = det::<1, 1>(8);
        assert_eq!(cache.get([5]), None);
    }

    #[test]
    fn put_overwrites_same_key() {
        let cache = det::<1, 1>(8);
        cache.put([3], [1]);
        cache.put([3], [2]);
        assert_eq!(cache.get([3]), Some([2]));
    }

    #[test]
    fn delete_existing_returns_true_then_absent() {
        let cache = det::<1, 1>(8);
        cache.put([3], [9]);
        assert!(cache.delete([3]));
        assert_eq!(cache.get([3]), None);
        // Deleting again finds an empty slot and reports nothing was removed.
        assert!(!cache.delete([3]));
    }

    #[test]
    fn delete_on_empty_slot_returns_false() {
        let cache = det::<1, 1>(8);
        assert!(!cache.delete([7]));
    }

    #[test]
    fn delete_wrong_key_in_occupied_slot_returns_false() {
        // size 4: keys 1 and 5 both map to slot 1 (1 % 4 == 5 % 4 == 1).
        let cache = det::<1, 1>(4);
        cache.put([1], [10]);
        // The slot holds key 1, so deleting key 5 must not touch it.
        assert!(!cache.delete([5]));
        assert_eq!(cache.get([1]), Some([10]));
    }

    #[test]
    fn colliding_put_evicts_previous_key() {
        // Keys 1 and 5 share slot 1; the second put overwrites the first.
        let cache = det::<1, 1>(4);
        cache.put([1], [100]);
        cache.put([5], [200]);
        assert_eq!(cache.get([1]), None);
        assert_eq!(cache.get([5]), Some([200]));
    }

    #[test]
    fn distinct_keys_in_distinct_slots() {
        let cache = det::<1, 1>(16);
        for k in 1u32..=10 {
            cache.put([k], [k * 2]);
        }
        for k in 1u32..=10 {
            assert_eq!(cache.get([k]), Some([k * 2]));
        }
    }

    #[test]
    fn size_one_slot_holds_one_entry() {
        let cache = det::<1, 1>(1);
        cache.put([1], [1]);
        assert_eq!(cache.get([1]), Some([1]));
        // Everything maps to the single slot, so a new key evicts.
        cache.put([2], [2]);
        assert_eq!(cache.get([1]), None);
        assert_eq!(cache.get([2]), Some([2]));
        assert!(cache.delete([2]));
        assert_eq!(cache.get([2]), None);
    }

    #[test]
    fn empty_key_is_a_reserved_sentinel() {
        // The empty key marks a free slot. On a fresh cache every slot already
        // "contains" the empty key, so looking it up yields the zero value.
        // Callers must keep the empty key out of their real key space.
        let cache = NbCacheWith::<FirstWordBuildHasher, 1, 1>::with_hasher_and_empty(
            8,
            FirstWordBuildHasher,
            [99],
        );
        assert_eq!(cache.get([99]), Some([0]));
        // A normal key is absent until inserted, then behaves as usual.
        assert_eq!(cache.get([1]), None);
        cache.put([1], [5]);
        assert_eq!(cache.get([1]), Some([5]));
        assert!(cache.delete([1]));
        assert_eq!(cache.get([1]), None);
    }

    #[test]
    fn with_empty_constructor_sets_sentinel() {
        let cache = NbCache::<1, 1>::with_empty(8, [77]);
        // Fresh slots report the sentinel key as present with a zero value.
        assert_eq!(cache.get([77]), Some([0]));
    }

    #[test]
    fn multi_word_keys_and_values_roundtrip() {
        // K = 2, V = 3 exercises the per-word loops more than once.
        let cache = det::<2, 3>(16);
        cache.put([1, 2], [10, 20, 30]);
        assert_eq!(cache.get([1, 2]), Some([10, 20, 30]));
        // Same slot (first word 1), different second word -> miss.
        assert_eq!(cache.get([1, 99]), None);
    }

    #[test]
    fn full_width_values_survive_tag_packing() {
        // Each word packs data in the low 32 bits and a tag in the high 32.
        // Round-tripping u32::MAX proves the data half is never clobbered.
        let cache = det::<2, 2>(8);
        cache.put([u32::MAX, 1], [u32::MAX, 7]);
        assert_eq!(cache.get([u32::MAX, 1]), Some([u32::MAX, 7]));
    }

    #[test]
    fn many_sequential_writes_flip_alternates() {
        // Repeatedly rewriting one slot advances the version/tag and flips
        // between the two alternates many times.
        let cache = det::<1, 1>(1);
        for i in 1u32..=1_000 {
            cache.put([1], [i]);
            assert_eq!(cache.get([1]), Some([i]));
        }
    }

    #[test]
    fn delete_then_reinsert() {
        let cache = det::<1, 1>(4);
        cache.put([1], [10]);
        assert!(cache.delete([1]));
        cache.put([1], [20]);
        assert_eq!(cache.get([1]), Some([20]));
    }

    #[test]
    #[should_panic(expected = "left != right")]
    fn zero_size_panics() {
        let _ = NbCache::<1, 1>::new(0);
    }

    /// Lock-free reads stay consistent while a *single* writer mutates the same
    /// slot. This drives the version-mismatch retry in `read_pair` (a reader
    /// catches the slot mid-flip) and the happy path of `write_pair` /
    /// `write_key_if_equal`, without provoking the concurrent-writer livelock
    /// documented on `write_pair` (a lone writer always makes progress).
    #[test]
    fn concurrent_readers_with_single_writer() {
        const VALUES: [u32; 4] = [11, 22, 33, 44];
        let cache = det::<1, 1>(1);
        let cache = &cache;

        std::thread::scope(|scope| {
            scope.spawn(move || {
                for i in 0..50_000usize {
                    cache.put([1], [VALUES[i % VALUES.len()]]);
                    if i % 2 == 0 {
                        cache.delete([1]);
                    }
                }
            });

            for _ in 0..3 {
                scope.spawn(move || {
                    for _ in 0..50_000 {
                        if let Some([v]) = cache.get([1]) {
                            assert!(
                                VALUES.contains(&v),
                                "reader observed a value that was never written: {v}",
                            );
                        }
                    }
                });
            }
        });
    }
}
