use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash, RandomState},
    iter,
    sync::{Mutex, RwLock},
};

use crate::{Cache, CacheType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoarseBlockingCacheType;

impl CacheType for CoarseBlockingCacheType {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FineBlockingCacheType;

impl CacheType for FineBlockingCacheType {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoarseRwBlockingCacheType;

impl CacheType for CoarseRwBlockingCacheType {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FineRwBlockingCacheType;

impl CacheType for FineRwBlockingCacheType {}

#[derive(Debug)]
pub struct CoarseBlockingCache<K, V> {
    inner: Mutex<HashMap<K, V>>,
}

impl<K, V> Default for CoarseBlockingCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> CoarseBlockingCache<K, V> {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }
}

impl<K, V> Cache for CoarseBlockingCache<K, V>
where
    K: Hash + Eq,
    V: Copy,
{
    type Type = CoarseBlockingCacheType;
    type Key = K;
    type Value = V;

    fn put(&self, key: Self::Key, value: Self::Value) {
        self.inner.lock().expect("poisoned lock during put").insert(key, value);
    }

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        self.inner.lock().expect("poisoned lock during get").get(&key).copied()
    }

    fn delete(&self, key: Self::Key) -> bool {
        self.inner
            .lock()
            .expect("poisoned lock during delete")
            .remove(&key)
            .is_some()
    }
}

type FineEntry<K, V> = Mutex<(K, V)>;

#[derive(Debug)]
pub struct FineBlockingCache<K, V, H = RandomState> {
    entries: Box<[FineEntry<K, V>]>,
    build_hasher: H,
    empty_key: K,
}

impl<K, V> FineBlockingCache<K, V>
where
    K: Hash + Eq + Copy,
    V: Copy + Default,
{
    pub fn new(capacity: usize) -> Self
    where
        K: Default,
    {
        Self::with_hasher(capacity, RandomState::new())
    }

    pub fn with_empty(capacity: usize, empty: K) -> Self {
        Self::with_hasher_and_empty(capacity, RandomState::new(), empty)
    }
}

impl<K, V, H> FineBlockingCache<K, V, H>
where
    H: BuildHasher,
    K: Hash + Eq + Copy,
    V: Copy + Default,
{
    pub fn with_hasher(capacity: usize, build_hasher: H) -> Self
    where
        K: Default,
    {
        Self::with_hasher_and_empty(capacity, build_hasher, K::default())
    }

    pub fn with_hasher_and_empty(
        capacity: usize,
        build_hasher: H,
        empty: K,
    ) -> Self {
        Self {
            entries: iter::repeat_with(|| Mutex::new((empty, V::default())))
                .take(capacity)
                .collect(),
            build_hasher,
            empty_key: empty,
        }
    }
}

impl<K, V, H> Cache for FineBlockingCache<K, V, H>
where
    H: BuildHasher,
    K: Hash + Eq + Copy,
    V: Copy,
{
    type Type = FineBlockingCacheType;
    type Key = K;
    type Value = V;

    fn put(&self, key: Self::Key, value: Self::Value) {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let mut guard =
            self.entries[index].lock().expect("poisoned lock during put");
        *guard = (key, value);
    }

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let guard =
            self.entries[index].lock().expect("poisoned lock during get");
        match *guard {
            (stored_key, stored_value) if stored_key == key => {
                Some(stored_value)
            },
            _ => None,
        }
    }

    fn delete(&self, key: Self::Key) -> bool {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let mut guard =
            self.entries[index].lock().expect("poisoned lock during delete");
        match *guard {
            (stored_key, value) if stored_key == key => {
                *guard = (self.empty_key, value);
                true
            },
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct CoarseRwBlockingCache<K, V> {
    inner: RwLock<HashMap<K, V>>,
}

impl<K, V> Default for CoarseRwBlockingCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> CoarseRwBlockingCache<K, V> {
    pub fn new() -> Self {
        Self { inner: RwLock::new(HashMap::new()) }
    }
}

impl<K, V> Cache for CoarseRwBlockingCache<K, V>
where
    K: Hash + Eq,
    V: Copy,
{
    type Type = CoarseRwBlockingCacheType;
    type Key = K;
    type Value = V;

    fn put(&self, key: Self::Key, value: Self::Value) {
        self.inner
            .write()
            .expect("poisoned lock during put")
            .insert(key, value);
    }

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        self.inner.read().expect("poisoned lock during get").get(&key).copied()
    }

    fn delete(&self, key: Self::Key) -> bool {
        self.inner
            .write()
            .expect("poisoned lock during delete")
            .remove(&key)
            .is_some()
    }
}

type FineRwEntry<K, V> = RwLock<Option<(K, V)>>;

#[derive(Debug)]
pub struct FineRwBlockingCache<K, V, H = RandomState> {
    entries: Box<[FineRwEntry<K, V>]>,
    build_hasher: H,
}

impl<K, V> FineRwBlockingCache<K, V>
where
    K: Hash + Eq + Copy,
    V: Copy,
{
    pub fn new(capacity: usize) -> Self {
        Self::with_hasher(capacity, RandomState::new())
    }
}

impl<K, V, H> FineRwBlockingCache<K, V, H>
where
    H: BuildHasher,
    K: Hash + Eq + Copy,
    V: Copy,
{
    pub fn with_hasher(capacity: usize, build_hasher: H) -> Self {
        Self {
            entries: iter::repeat_with(|| RwLock::new(None))
                .take(capacity)
                .collect(),
            build_hasher,
        }
    }
}

impl<K, V, H> Cache for FineRwBlockingCache<K, V, H>
where
    H: BuildHasher,
    K: Hash + Eq + Copy,
    V: Copy,
{
    type Type = FineRwBlockingCacheType;
    type Key = K;
    type Value = V;

    fn put(&self, key: Self::Key, value: Self::Value) {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let mut guard =
            self.entries[index].write().expect("poisoned lock during put");
        *guard = Some((key, value));
    }

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let guard =
            self.entries[index].read().expect("poisoned lock during get");
        match *guard {
            Some((stored_key, stored_value)) if stored_key == key => {
                Some(stored_value)
            },
            _ => None,
        }
    }

    fn delete(&self, key: Self::Key) -> bool {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let mut guard =
            self.entries[index].write().expect("poisoned lock during delete");
        match *guard {
            Some((stored_key, _)) if stored_key == key => {
                *guard = None;
                true
            },
            _ => false,
        }
    }
}
