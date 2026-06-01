use std::hash::{BuildHasher, RandomState};

use entry::Entry;

use crate::{Cache, CacheType};

mod entry;

pub type OfKey<const K: usize> = [u32; K];
pub type OfValue<const V: usize> = [u32; V];

pub type OfCache<const K: usize, const V: usize> =
    OfCacheWith<RandomState, K, V>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfCacheType;

impl CacheType for OfCacheType {}

#[derive(Debug)]
pub struct OfCacheWith<H, const K: usize, const V: usize> {
    entries: Box<[Entry<K, V>]>,
    build_hasher: H,
    empty_key: OfKey<K>,
}

impl<const K: usize, const V: usize> OfCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self::with_hasher(capacity, RandomState::new())
    }

    pub fn with_empty(capacity: usize, empty_key: OfKey<K>) -> Self {
        Self::with_hasher_and_empty(capacity, RandomState::new(), empty_key)
    }
}

impl<H, const K: usize, const V: usize> OfCacheWith<H, K, V>
where
    H: BuildHasher,
{
    pub fn with_hasher(capacity: usize, build_hasher: H) -> Self {
        Self::with_hasher_and_empty(capacity, build_hasher, [0; K])
    }

    pub fn with_hasher_and_empty(
        capacity: usize,
        build_hasher: H,
        empty_key: OfKey<K>,
    ) -> Self {
        assert_ne!(capacity, 0);

        let mut entries = Vec::with_capacity(capacity);
        for _ in 0 .. capacity {
            entries.push(Entry::new(empty_key));
        }
        Self { entries: entries.into(), build_hasher, empty_key }
    }
}

impl<H, const K: usize, const V: usize> Cache for OfCacheWith<H, K, V>
where
    H: BuildHasher,
{
    type Key = OfKey<K>;
    type Value = OfValue<V>;
    type Type = OfCacheType;

    fn put(&self, key: Self::Key, value: Self::Value) {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        self.entries[index].write_pair(key, value);
    }

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        let (stored_key, stored_value) = self.entries[index].read_pair();
        if stored_key == key { Some(stored_value) } else { None }
    }

    fn delete(&self, key: Self::Key) -> bool {
        let hash = self.build_hasher.hash_one(key);
        let size = self.entries.len() as u64;
        let index = (hash % size) as usize;
        self.entries[index].write_key_if_equal(key, self.empty_key)
    }
}

#[cfg(feature = "zeroize")]
impl<H, const K: usize, const V: usize> zeroize::Zeroize
    for OfCacheWith<H, K, V>
{
    fn zeroize(&mut self) {
        self.entries.zeroize();
    }
}

#[cfg(test)]
mod test {
    use crate::Cache;

    use super::OfCache;

    #[test]
    fn empty_cache_contains_nothing() {
        let cache = OfCache::<3, 2>::new(31);
        assert_eq!(cache.get([1, 2, 3]), None);
        assert_eq!(cache.get([0, 0, 1]), None);
        assert_eq!(cache.get([u32::MAX, u32::MAX - 1, u32::MAX - 2]), None);
    }

    #[test]
    fn cache_with_one_thing_contains_that_thing() {
        let cache = OfCache::<3, 2>::new(31);
        cache.put([1, 2, 3], [5, 6]);
        assert_eq!(cache.get([1, 2, 3]), Some([5, 6]));
        assert_eq!(cache.get([0, 0, 1]), None);
        assert_eq!(cache.get([u32::MAX, u32::MAX - 1, u32::MAX - 2]), None);
    }

    #[test]
    fn cache_with_multiple_things_must_still_contain_the_last_one() {
        let cache = OfCache::<3, 2>::new(5);
        cache.put([1, 2, 3], [5, 6]);
        cache.put([10, 2, 3], [50, 6]);
        cache.put([1, 20, 3], [5, 60]);
        cache.put([1, 2, 30], [50, 60]);
        cache.put([10, 20, 3], [7, 3]);
        cache.put([1, 20, 30], [8, u32::MAX / 3]);
        assert_eq!(cache.get([1, 20, 30]), Some([8, u32::MAX / 3]));
    }

    #[test]
    fn cache_with_multiple_things_must_not_invent_keys() {
        let cache = OfCache::<3, 2>::new(5);
        cache.put([1, 2, 3], [5, 6]);
        cache.put([10, 2, 3], [50, 6]);
        cache.put([1, 20, 3], [5, 60]);
        cache.put([1, 2, 30], [50, 60]);
        cache.put([10, 20, 3], [7, 3]);
        cache.put([1, 20, 30], [8, u32::MAX / 3]);
        assert_eq!(cache.get([9, 93, 903]), None);
        assert_eq!(cache.get([u32::MAX >> 2, 2453, 234325]), None);
    }

    #[test]
    fn delete_must_make_entry_return_none() {
        let cache = OfCache::<3, 2>::new(5);
        cache.put([1, 2, 3], [5, 6]);
        cache.put([10, 2, 3], [50, 6]);
        cache.put([1, 20, 3], [5, 60]);
        cache.put([1, 2, 30], [50, 60]);
        cache.put([10, 20, 3], [7, 3]);
        cache.put([1, 20, 30], [8, u32::MAX / 3]);
        let deleted = cache.delete([1, 20, 30]);
        assert!(deleted);
        assert_eq!(cache.get([1, 20, 30]), None);
    }

    #[test]
    fn delete_must_make_remove_only_one_entry() {
        let cache = OfCache::<3, 2>::new(3);
        cache.put([1, 2, 3], [5, 6]);
        cache.put([1, 2, 30], [50, 60]);
        cache.put([10, 20, 3], [7, 3]);
        cache.put([1, 20, 30], [8, u32::MAX / 3]);
        let mut hits_then = 0;
        for key in [[1, 2, 3], [10, 20, 3], [1, 2, 30], [1, 20, 30]] {
            if cache.get(key).is_some() {
                hits_then += 1;
            }
        }
        cache.delete([1, 20, 30]);
        let mut hits_now = 0;
        for key in [[1, 2, 3], [10, 20, 3], [1, 2, 30], [1, 20, 30]] {
            if cache.get(key).is_some() {
                hits_now += 1;
            }
        }
        assert_eq!(hits_now + 1, hits_then);
    }

    #[test]
    fn empty_key_is_writable_default() {
        let cache = OfCache::<3, 2>::new(3);
        cache.put([0, 0, 0], [5, 6]);
        assert_eq!(cache.get([0, 0, 0]), Some([5, 6]));
    }

    #[test]
    fn empty_key_is_writable_custom() {
        let cache = OfCache::<3, 2>::with_empty(3, [3, 2, 1]);
        cache.put([3, 2, 1], [5, 6]);
        assert_eq!(cache.get([3, 2, 1]), Some([5, 6]));
    }

    #[test]
    fn empty_key_is_not_fully_deletable_default() {
        let cache = OfCache::<3, 2>::new(3);
        cache.put([0, 0, 0], [5, 6]);
        assert!(cache.delete([0, 0, 0]));
        assert!(cache.delete([0, 0, 0]));
        assert_eq!(cache.get([0, 0, 0]), Some([0, 0]));
    }

    #[test]
    fn empty_key_is_not_fully_deletable_custom() {
        let cache = OfCache::<3, 2>::with_empty(3, [3, 2, 1]);
        cache.put([3, 2, 1], [5, 6]);
        assert!(cache.delete([3, 2, 1]));
        assert!(cache.delete([3, 2, 1]));
        assert_eq!(cache.get([3, 2, 1]), Some([0, 0]));
    }
}
