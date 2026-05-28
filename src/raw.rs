use std::{
    hash::{BuildHasher, RandomState},
    sync::atomic::{AtomicU64, Ordering},
};

pub type Key<const K: usize> = [u32; K];
pub type Value<const V: usize> = [u32; V];

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
                    .compare_exchange(
                        data,
                        new_data,
                        Ordering::Release,
                        Ordering::Relaxed,
                    )
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
            for (dest, src) in alternate.key.iter().zip(new.into_iter()) {
                let data = dest.load(Ordering::Relaxed);
                let embedded_tag = data >> 32;
                let prev_tag_low = prev_tag & 0xff_ff_ff_ff;
                if embedded_tag != prev_tag_low {
                    outdated = true;
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
                if embedded_tag != prev_tag_low {
                    outdated = true;
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
