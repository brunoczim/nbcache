#[cfg(not(feature = "loom"))]
use std::sync::Mutex;
use std::{collections::HashMap, hash::Hash};

#[cfg(feature = "loom")]
use loom::sync::Mutex;

use crate::{Cache, CacheType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockingCacheType;

impl CacheType for BlockingCacheType {}

#[derive(Debug)]
pub struct BlockingCache<K, V> {
    inner: Mutex<HashMap<K, V>>,
}

impl<K, V> Default for BlockingCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> BlockingCache<K, V> {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }
}

impl<K, V> Cache for BlockingCache<K, V>
where
    K: Hash + Eq,
    V: Copy,
{
    type Type = BlockingCacheType;
    type Key = K;
    type Value = V;

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        self.inner.lock().expect("poisoned lock during get").get(&key).copied()
    }

    fn put(&self, key: Self::Key, value: Self::Value) {
        self.inner.lock().expect("poisoned lock during put").insert(key, value);
    }

    fn delete(&self, key: Self::Key) -> bool {
        self.inner
            .lock()
            .expect("poisoned lock during delete")
            .remove(&key)
            .is_some()
    }
}
