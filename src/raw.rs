use std::{
    hash::{BuildHasher, RandomState},
    sync::atomic::{AtomicU64, Ordering},
};

pub type Key<const K: usize> = [u32; K];
pub type Value<const V: usize> = [u32; V];

fn tag_preceeds(this: u32, that: u32) -> bool {
    const HALF_DISTANCE: u32 = u32::MAX >> 1;

    if this <= that {
        that - this <= HALF_DISTANCE
    } else {
        this - that > HALF_DISTANCE
    }
}

pub type NbCache<const K: usize, const V: usize> =
    NbCacheWith<RandomState, K, V>;

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

    pub fn with_hasher_and_empty(
        size: usize,
        build_hasher: H,
        empty_key: Key<K>,
    ) -> Self {
        assert_ne!(size, 0);

        let mut entries = Vec::with_capacity(size);
        for _ in 0 .. size {
            entries.push(Entry::new(empty_key));
        }
        Self { entries: entries.into(), build_hasher, empty_key }
    }

    pub fn get(&self, key: Key<K>) -> Option<Value<V>> {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let (stored_key, stored_value) = self.entries[index].read_pair();
        if stored_key == key { Some(stored_value) } else { None }
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
            let alternate_index = (curr_version >> 63) as usize;
            let tag = curr_version & 0xff_ff_ff_ff;
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

    pub fn read_key(&self) -> Key<K> {
        let mut curr_version = self.version.load(Ordering::Acquire);

        'main: loop {
            let alternate_index = (curr_version >> 63) as usize;
            let tag = curr_version & 0xff_ff_ff_ff;
            let alternate = &self.alternates[alternate_index];

            let mut key = [0; K];

            for (dest, src) in key.iter_mut().zip(alternate.key.iter()) {
                let data = src.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let tag_low = tag & 0xff_ff_ff_ff;
                if !tag_preceeds(tag_low as u32, embedded_tag as u32) {
                    curr_version = self.version.load(Ordering::Acquire);
                    continue 'main;
                }
                *dest = data as u32;
            }

            break key;
        }
    }

    pub fn write_pair(&self, key: Key<K>, value: Value<V>) {
        'main: loop {
            let (prev_version, curr_version) = self.start_write();

            let alternate_index = (prev_version >> 63) as usize;
            let prev_tag = prev_version & 0xff_ff_ff_ff;
            let prev_tag_low = prev_tag & 0xff_ff_ff_ff;
            let next_alt_index = 1 - alternate_index;
            let alternate = &self.alternates[next_alt_index];
            let next_tag = curr_version & 0xff_ff_ff_ff;

            for (dest, src) in alternate
                .key
                .iter()
                .chain(&alternate.value)
                .zip(key.into_iter().chain(value))
            {
                let data = dest.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                if !tag_preceeds(embedded_tag as u32, prev_tag_low as u32) {
                    continue 'main;
                }
                let new_data = u64::from(src) | (next_tag << 32);
                if dest
                    .compare_exchange(
                        data,
                        new_data,
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
                    .is_err()
                {
                    continue 'main;
                }
            }

            let next_version = next_tag | ((next_alt_index as u64) << 63);
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
                break;
            }
        }
    }

    pub fn write_key_if_equal(&self, expected: Key<K>, new: Key<K>) -> bool {
        'main: loop {
            let curr_key = self.read_key();
            if curr_key != expected {
                return false;
            }

            let (prev_version, curr_version) = self.start_write();

            let alternate_index = (prev_version >> 63) as usize;
            let prev_tag = prev_version & 0xff_ff_ff_ff;
            let prev_tag_low = prev_tag & 0xff_ff_ff_ff;
            let next_alt_index = 1 - alternate_index;
            let alternate = &self.alternates[next_alt_index];
            let next_tag = curr_version & 0xff_ff_ff_ff;

            for (dest, src) in alternate.key.iter().zip(new.into_iter()) {
                let data = dest.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                if !tag_preceeds(embedded_tag as u32, prev_tag_low as u32) {
                    continue 'main;
                }
                let new_data = u64::from(src) | (next_tag << 32);
                if dest
                    .compare_exchange(
                        data,
                        new_data,
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
                    .is_err()
                {
                    continue 'main;
                }
            }

            for dest in alternate.value.iter() {
                let data = dest.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let prev_tag_low = prev_tag & 0xff_ff_ff_ff;
                if !tag_preceeds(embedded_tag as u32, prev_tag_low as u32) {
                    continue 'main;
                }
                let new_data = next_tag << 32;
                if dest
                    .compare_exchange(
                        data,
                        new_data,
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
                    .is_err()
                {
                    continue 'main;
                }
            }

            let next_version = next_tag | ((next_alt_index as u64) << 63);
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

    fn start_write(&self) -> (u64, u64) {
        let current = self.version.load(Ordering::Acquire);
        self.start_write_from(current)
    }

    fn start_write_from(&self, mut current: u64) -> (u64, u64) {
        loop {
            let next = ((current + 1) & 0xff_ff_ff_ff)
                | (current & (0xff_ff_ff_ff << 32));

            match self.version.compare_exchange(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(prev) => break (prev, next),
                Err(actual) => current = actual,
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
