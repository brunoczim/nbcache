pub mod of;
pub mod transforming;

pub trait CacheType {}

pub trait Cache {
    type Key;
    type Value;
    type Type: CacheType;

    fn get(&self, key: Self::Key) -> Option<Self::Value>;

    fn put(&self, key: Self::Key, value: Self::Value);

    fn delete(&self, key: Self::Key) -> bool;
}
