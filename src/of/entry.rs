#[cfg(not(feature = "loom"))]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "loom")]
use loom::sync::atomic::{AtomicU64, Ordering};

use super::{OfKey, OfValue};

fn tag_preceeds(this: u32, that: u32) -> bool {
    const HALF_DISTANCE: u32 = u32::MAX >> 1;

    if this <= that {
        that - this <= HALF_DISTANCE
    } else {
        this - that > HALF_DISTANCE
    }
}

#[derive(Debug)]
pub struct Entry<const K: usize, const V: usize> {
    version: AtomicU64,
    alternates: [EntryAlternate<K, V>; 2],
}

impl<const K: usize, const V: usize> Entry<K, V> {
    pub fn new(key: OfKey<K>) -> Self {
        Self {
            version: AtomicU64::new(0),
            alternates: [
                EntryAlternate::new(key, [0; V], 0),
                EntryAlternate::new(key, [0; V], u32::MAX),
            ],
        }
    }

    pub fn read_pair(&self) -> (OfKey<K>, OfValue<V>) {
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

    pub fn read_key_versioned(&self) -> (OfKey<K>, u64) {
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

            break (key, curr_version);
        }
    }

    pub fn write_pair(&self, key: OfKey<K>, value: OfValue<V>) {
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

    pub fn write_key_if_equal(
        &self,
        expected: OfKey<K>,
        new: OfKey<K>,
    ) -> bool {
        'main: loop {
            let (prev_version, curr_version) = loop {
                let (curr_key, curr_version) = self.read_key_versioned();
                if curr_key != expected {
                    return false;
                }

                if let Ok(tuple) = self.start_write_weak(curr_version) {
                    break tuple;
                }
            };

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
            match self.start_write_weak(current) {
                Ok((prev, next)) => break (prev, next),
                Err(actual) => current = actual,
            }
        }
    }

    fn start_write_weak(&self, current: u64) -> Result<(u64, u64), u64> {
        let next =
            ((current + 1) & 0xff_ff_ff_ff) | (current & (0xff_ff_ff_ff << 32));

        match self.version.compare_exchange(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(prev) => Ok((prev, next)),
            Err(actual) => Err(actual),
        }
    }
}

#[cfg(feature = "zeroize")]
impl<const K: usize, const V: usize> zeroize::Zeroize for Entry<K, V> {
    fn zeroize(&mut self) {
        self.alternates.zeroize();
    }
}

#[derive(Debug)]
struct EntryAlternate<const K: usize, const V: usize> {
    key: [AtomicU64; K],
    value: [AtomicU64; V],
}

impl<const K: usize, const V: usize> EntryAlternate<K, V> {
    pub fn new(key: OfKey<K>, value: OfValue<V>, tag: u32) -> Self {
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

#[cfg(feature = "zeroize")]
impl<const K: usize, const V: usize> zeroize::Zeroize for EntryAlternate<K, V> {
    fn zeroize(&mut self) {
        self.key.iter().for_each(|elem| elem.store(0, Ordering::Relaxed));
        self.value.iter().for_each(|elem| elem.store(0, Ordering::Relaxed));
    }
}

#[cfg(test)]
mod test {
    use super::{Entry, tag_preceeds};

    #[test]
    fn same_tag_preceeds() {
        let has_relation = tag_preceeds(40, 40);
        assert!(has_relation);
    }

    #[test]
    fn one_below_tag_preceeds() {
        let has_relation = tag_preceeds(39, 40);
        assert!(has_relation);
    }

    #[test]
    fn far_below_tag_preceeds() {
        let has_relation = tag_preceeds(0, 40);
        assert!(has_relation);
    }

    #[test]
    fn far_below_tag_preceeds_wrap_around() {
        let has_relation = tag_preceeds(u32::MAX - 1, 40);
        assert!(has_relation);
    }

    #[test]
    fn far_below_tag_preceeds_near_limit() {
        let has_relation = tag_preceeds(0, u32::MAX >> 1);
        assert!(has_relation);
    }

    #[test]
    fn far_below_tag_preceeds_near_limit_wrap_around() {
        let has_relation = tag_preceeds(u32::MAX, u32::MAX >> 1);
        assert!(has_relation);
    }

    #[test]
    fn one_above_tag_preceeds_false() {
        let has_relation = tag_preceeds(41, 40);
        assert!(!has_relation);
    }

    #[test]
    fn far_above_tag_preceeds_false() {
        let has_relation = tag_preceeds(100, 40);
        assert!(!has_relation);
    }

    #[test]
    fn far_above_tag_preceeds_wrap_around_false() {
        let has_relation = tag_preceeds(100, u32::MAX - 1);
        assert!(!has_relation);
    }

    #[test]
    fn far_above_tag_preceeds_near_limit_false() {
        let has_relation = tag_preceeds(0, (u32::MAX >> 1) + 1);
        assert!(!has_relation);
    }

    #[test]
    fn far_below_tag_preceeds_near_limit_wrap_around_false() {
        let has_relation = tag_preceeds(u32::MAX, (u32::MAX >> 1) + 1);
        assert!(!has_relation);
    }

    #[test]
    fn read_pair_empty() {
        let entry = Entry::new([1, 2]);
        let (key, value) = entry.read_pair();
        assert_eq!(key, [1, 2]);
        assert_eq!(value, [0, 0]);
    }

    #[test]
    fn read_key_versioned_empty() {
        let entry = Entry::<2, 1>::new([1, 2]);
        let (key, version) = entry.read_key_versioned();
        assert_eq!(key, [1, 2]);
        assert_eq!(version, 0);
    }

    #[test]
    fn write_pair_entails_read_pair() {
        let entry = Entry::new([0; 4]);
        entry.write_pair([1, 2, 3, 40], [3, 4, 5]);
        let (key, value) = entry.read_pair();
        assert_eq!(key, [1, 2, 3, 40]);
        assert_eq!(value, [3, 4, 5]);
    }

    #[test]
    fn write_pair_entails_read_key_versioned() {
        let entry = Entry::new([0; 4]);
        entry.write_pair([1, 2, 3, 40], [3, 4, 5]);
        let (key, version) = entry.read_key_versioned();
        assert_eq!(key, [1, 2, 3, 40]);
        assert_eq!(version, 1 | (1 << 63));
    }

    #[test]
    fn write_key_if_equal_success() {
        let entry = Entry::<4, 1>::new([10, u32::MAX - 4, 3, 4]);
        let success = entry
            .write_key_if_equal([10, u32::MAX - 4, 3, 4], [221, 232, 10, 20]);
        assert!(success);
    }

    #[test]
    fn write_key_if_equal_failure() {
        let entry = Entry::<4, 1>::new([10, u32::MAX - 4, 3, 4]);
        let success = entry
            .write_key_if_equal([0, u32::MAX - 4, 3, 4], [221, 232, 10, 20]);
        assert!(!success);
    }

    #[test]
    fn write_key_if_equal_success_entails_read_pair() {
        let entry = Entry::new([10, u32::MAX - 4, 3, 4]);
        entry.write_key_if_equal([10, u32::MAX - 4, 3, 4], [221, 232, 10, 20]);
        let (key, value) = entry.read_pair();
        assert_eq!(key, [221, 232, 10, 20]);
        assert_eq!(value, [0]);
    }

    #[test]
    fn write_key_if_equal_success_entails_read_key_versioned() {
        let entry = Entry::<4, 1>::new([10, u32::MAX - 4, 3, 4]);
        entry.write_key_if_equal([10, u32::MAX - 4, 3, 4], [221, 232, 10, 20]);
        let (key, version) = entry.read_key_versioned();
        assert_eq!(key, [221, 232, 10, 20]);
        assert_eq!(version, 1 | (1 << 63));
    }
}
