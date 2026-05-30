use std::marker::PhantomData;

use crate::{Cache, CacheType, of::OfCacheType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformingCacheType<C>(pub C);

impl<C> CacheType for TransformingCacheType<C> where C: CacheType {}

#[derive(Debug, Clone)]
pub struct TransformingCache<K, V, C> {
    raw: C,
    _marker: PhantomData<(K, V)>,
}

impl<K, V, C> TransformingCache<K, V, C> {
    pub fn new(raw: C) -> Self {
        Self { raw, _marker: PhantomData }
    }

    pub fn raw(&self) -> &C {
        &self.raw
    }

    pub fn into_raw(self) -> C {
        self.raw
    }
}

impl<K, V, C> Cache for TransformingCache<K, V, C>
where
    C: Cache,
    K: TransformFor<C::Type, Transformed = C::Key>,
    V: TransformFor<C::Type, Transformed = C::Value>,
    K: TransformInto<K::Transformed>,
    V: TransformInto<V::Transformed> + TransformFrom<V::Transformed>,
{
    type Key = K;
    type Value = V;
    type Type = TransformingCacheType<C::Type>;

    fn get(&self, key: K) -> Option<V> {
        self.raw.get(key.encode()).map(V::decode)
    }

    fn put(&self, key: K, value: V) {
        self.raw.put(key.encode(), value.encode());
    }

    fn delete(&self, key: K) -> bool {
        self.raw.delete(key.encode())
    }
}

pub trait TransformFor<T>
where
    T: CacheType,
{
    type Transformed;
}

pub trait TransformInto<T> {
    fn encode(self) -> T;
}

pub trait TransformFrom<T> {
    fn decode(value: T) -> Self;
}

impl TransformInto<[u32; 1]> for bool {
    fn encode(self) -> [u32; 1] {
        [self as u32]
    }
}

impl TransformFrom<[u32; 1]> for bool {
    fn decode(value: [u32; 1]) -> Self {
        value[0] != 0
    }
}

impl TransformInto<[u32; 1]> for u8 {
    fn encode(self) -> [u32; 1] {
        [self as u32]
    }
}

impl TransformFrom<[u32; 1]> for u8 {
    fn decode(value: [u32; 1]) -> Self {
        value[0] as Self
    }
}

impl TransformInto<[u32; 1]> for i8 {
    fn encode(self) -> [u32; 1] {
        (self as u8).encode()
    }
}

impl TransformFrom<[u32; 1]> for i8 {
    fn decode(value: [u32; 1]) -> Self {
        u8::decode(value) as Self
    }
}

impl TransformInto<[u32; 1]> for u16 {
    fn encode(self) -> [u32; 1] {
        [self as u32]
    }
}

impl TransformFrom<[u32; 1]> for u16 {
    fn decode(value: [u32; 1]) -> Self {
        value[0] as Self
    }
}

impl TransformInto<[u32; 1]> for i16 {
    fn encode(self) -> [u32; 1] {
        (self as u16).encode()
    }
}

impl TransformFrom<[u32; 1]> for i16 {
    fn decode(value: [u32; 1]) -> Self {
        u16::decode(value) as Self
    }
}

impl TransformInto<[u32; 1]> for u32 {
    fn encode(self) -> [u32; 1] {
        [self]
    }
}

impl TransformFrom<[u32; 1]> for u32 {
    fn decode(value: [u32; 1]) -> Self {
        value[0]
    }
}

impl TransformInto<[u32; 1]> for i32 {
    fn encode(self) -> [u32; 1] {
        (self as u32).encode()
    }
}

impl TransformFrom<[u32; 1]> for i32 {
    fn decode(value: [u32; 1]) -> Self {
        u32::decode(value) as Self
    }
}

impl TransformInto<[u32; 1]> for f32 {
    fn encode(self) -> [u32; 1] {
        [self.to_bits()]
    }
}

impl TransformFrom<[u32; 1]> for f32 {
    fn decode(value: [u32; 1]) -> Self {
        Self::from_bits(value[0])
    }
}

impl TransformInto<[u32; 1]> for char {
    fn encode(self) -> [u32; 1] {
        [self as u32]
    }
}

impl TransformFrom<[u32; 1]> for char {
    fn decode(value: [u32; 1]) -> Self {
        char::try_from(value[0]).unwrap_or(char::REPLACEMENT_CHARACTER)
    }
}

impl TransformInto<[u32; 2]> for u64 {
    fn encode(self) -> [u32; 2] {
        [self as u32, (self >> 32) as u32]
    }
}

impl TransformFrom<[u32; 2]> for u64 {
    fn decode(value: [u32; 2]) -> Self {
        value[0] as Self | ((value[1] as Self) << 32)
    }
}

impl TransformInto<[u32; 3]> for u64 {
    fn encode(self) -> [u32; 3] {
        [self as u32, (self >> 32) as u32, 0]
    }
}

impl TransformInto<[u32; 2]> for i64 {
    fn encode(self) -> [u32; 2] {
        (self as u64).encode()
    }
}

impl TransformFrom<[u32; 2]> for i64 {
    fn decode(value: [u32; 2]) -> Self {
        u64::decode(value) as Self
    }
}

impl TransformInto<[u32; 2]> for f64 {
    fn encode(self) -> [u32; 2] {
        self.to_bits().encode()
    }
}

impl TransformFrom<[u32; 2]> for f64 {
    fn decode(value: [u32; 2]) -> Self {
        Self::from_bits(u64::decode(value))
    }
}

impl TransformInto<[u32; 4]> for u128 {
    fn encode(self) -> [u32; 4] {
        [
            self as u32,
            (self >> 32) as u32,
            (self >> 64) as u32,
            (self >> 96) as u32,
        ]
    }
}

impl TransformFrom<[u32; 4]> for u128 {
    fn decode(value: [u32; 4]) -> Self {
        value[0] as Self
            | ((value[1] as Self) << 32)
            | ((value[2] as Self) << 64)
            | ((value[3] as Self) << 96)
    }
}

impl TransformInto<[u32; 4]> for i128 {
    fn encode(self) -> [u32; 4] {
        (self as u128).encode()
    }
}

impl TransformFrom<[u32; 4]> for i128 {
    fn decode(value: [u32; 4]) -> Self {
        u128::decode(value) as Self
    }
}

impl<const N: usize> TransformFrom<[u32; N]> for [u32; N] {
    fn decode(value: [u32; N]) -> Self {
        value
    }
}

impl<const N: usize> TransformInto<[u32; N]> for [u32; N] {
    fn encode(self) -> [u32; N] {
        self
    }
}

impl TransformInto<[u32; 1]> for [u8; 2] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u32 | (self[1] as u32) << 8]
    }
}

impl TransformFrom<[u32; 1]> for [u8; 2] {
    fn decode(value: [u32; 1]) -> Self {
        [value[0] as u8, (value[0] >> 8) as u8]
    }
}

impl TransformInto<[u32; 1]> for [u8; 3] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u32 | (self[1] as u32) << 8 | (self[2] as u32) << 16]
    }
}

impl TransformFrom<[u32; 1]> for [u8; 3] {
    fn decode(value: [u32; 1]) -> Self {
        [value[0] as u8, (value[0] >> 8) as u8, (value[0] >> 16) as u8]
    }
}

impl TransformInto<[u32; 1]> for [u8; 4] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u32
            | (self[1] as u32) << 8
            | (self[2] as u32) << 16
            | (self[3] as u32) << 24]
    }
}

impl TransformFrom<[u32; 1]> for [u8; 4] {
    fn decode(value: [u32; 1]) -> Self {
        [
            value[0] as u8,
            (value[0] >> 8) as u8,
            (value[0] >> 16) as u8,
            (value[0] >> 24) as u8,
        ]
    }
}

impl TransformInto<[u32; 1]> for [i8; 2] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u8 as u32 | (self[1] as u8 as u32) << 8]
    }
}

impl TransformFrom<[u32; 1]> for [i8; 2] {
    fn decode(value: [u32; 1]) -> Self {
        [value[0] as u8 as i8, (value[0] >> 8) as u8 as i8]
    }
}

impl TransformInto<[u32; 1]> for [i8; 3] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u8 as u32
            | (self[1] as u8 as u32) << 8
            | (self[2] as u8 as u32) << 16]
    }
}

impl TransformFrom<[u32; 1]> for [i8; 3] {
    fn decode(value: [u32; 1]) -> Self {
        [
            value[0] as u8 as i8,
            (value[0] >> 8) as u8 as i8,
            (value[0] >> 16) as u8 as i8,
        ]
    }
}

impl TransformInto<[u32; 1]> for [i8; 4] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u8 as u32
            | (self[1] as u8 as u32) << 8
            | (self[2] as u8 as u32) << 16
            | (self[3] as u8 as u32) << 24]
    }
}

impl TransformFrom<[u32; 1]> for [i8; 4] {
    fn decode(value: [u32; 1]) -> Self {
        [
            value[0] as u8 as i8,
            (value[0] >> 8) as u8 as i8,
            (value[0] >> 16) as u8 as i8,
            (value[0] >> 24) as u8 as i8,
        ]
    }
}

impl TransformInto<[u32; 1]> for [u16; 2] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u32 | (self[1] as u32) << 16]
    }
}

impl TransformFrom<[u32; 1]> for [u16; 2] {
    fn decode(value: [u32; 1]) -> Self {
        [value[0] as u16, (value[0] >> 16) as u16]
    }
}

impl TransformInto<[u32; 1]> for [i16; 2] {
    fn encode(self) -> [u32; 1] {
        [self[0] as u16 as u32 | (self[1] as u16 as u32) << 16]
    }
}

impl TransformFrom<[u32; 1]> for [i16; 2] {
    fn decode(value: [u32; 1]) -> Self {
        [value[0] as u16 as i16, (value[0] >> 16) as u16 as i16]
    }
}

#[cfg(feature = "uuid")]
impl TransformInto<[u32; 4]> for uuid::Uuid {
    fn encode(self) -> [u32; 4] {
        self.as_u128().encode()
    }
}

#[cfg(feature = "uuid")]
impl TransformFrom<[u32; 4]> for uuid::Uuid {
    fn decode(value: [u32; 4]) -> Self {
        Self::from_u128(u128::decode(value))
    }
}

impl TransformFor<OfCacheType> for bool {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for u8 {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for i8 {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for u16 {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for i16 {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for u32 {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for i32 {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for f32 {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for char {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for u64 {
    type Transformed = [u32; 2];
}

impl TransformFor<OfCacheType> for i64 {
    type Transformed = [u32; 2];
}

impl TransformFor<OfCacheType> for f64 {
    type Transformed = [u32; 2];
}

impl TransformFor<OfCacheType> for u128 {
    type Transformed = [u32; 4];
}

impl TransformFor<OfCacheType> for i128 {
    type Transformed = [u32; 4];
}

impl TransformFor<OfCacheType> for [u8; 2] {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for [u8; 3] {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for [u8; 4] {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for [i8; 2] {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for [i8; 3] {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for [i8; 4] {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for [u16; 2] {
    type Transformed = [u32; 1];
}

impl TransformFor<OfCacheType> for [i16; 2] {
    type Transformed = [u32; 1];
}

#[cfg(feature = "uuid")]
impl TransformFor<OfCacheType> for uuid::Uuid {
    type Transformed = [u32; 4];
}

#[cfg(test)]
mod test {
    use crate::{
        Cache,
        of::{OfCache, OfCacheType},
        transforming::TransformFor,
    };

    use super::{TransformFrom, TransformInto, TransformingCache};

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
    )]
    struct MultipleChoices(u8);

    impl TransformFrom<[u32; 1]> for MultipleChoices {
        fn decode(value: [u32; 1]) -> Self {
            Self(value[0] as u8)
        }
    }

    impl TransformInto<[u32; 1]> for MultipleChoices {
        fn encode(self) -> [u32; 1] {
            [self.0 as u32]
        }
    }

    impl TransformFrom<[u32; 2]> for MultipleChoices {
        fn decode(value: [u32; 2]) -> Self {
            Self(value[0] as u8)
        }
    }

    impl TransformInto<[u32; 2]> for MultipleChoices {
        fn encode(self) -> [u32; 2] {
            [self.0 as u32, 0]
        }
    }

    impl TransformFor<OfCacheType> for MultipleChoices {
        type Transformed = [u32; 1];
    }

    #[test]
    fn should_be_able_to_decide_key() {
        let cache = TransformingCache::new(OfCache::new(5));
        cache.put(MultipleChoices(3), 4_u8);
    }

    #[test]
    fn should_be_able_to_decide_value() {
        let cache = TransformingCache::new(OfCache::new(5));
        cache.put(4_u8, MultipleChoices(3));
    }

    #[test]
    fn should_be_able_to_decide_both_key_value() {
        let cache = TransformingCache::new(OfCache::new(5));
        cache.put(MultipleChoices(4), MultipleChoices(3));
    }

    #[test]
    fn put_should_get_the_same() {
        let cache = TransformingCache::new(OfCache::new(5));
        cache.put(0x01234567_89abcdef_u64, [3_u8, 2]);
        assert_eq!(cache.get(0x01234567_89abcdef,), Some([3_u8, 2]));
    }

    #[test]
    fn remove_after_put_should_be_successful() {
        let cache = TransformingCache::new(OfCache::new(5));
        cache.put(0x01234567_89abcdef_u64, [3_u8, 2]);
        assert!(cache.delete(0x01234567_89abcdef_u64,),);
    }

    #[test]
    fn encode_u8_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode(120_u8);
        assert_eq!(output, [120]);
    }

    #[test]
    fn encode_u8_from_u32_array() {
        let output = u8::decode([120_u32]);
        assert_eq!(output, 120);
    }

    #[test]
    fn encode_i8_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode(-2_i8);
        assert_eq!(output, [254]);
    }

    #[test]
    fn encode_i8_from_u32_array() {
        let output = i8::decode([254_u32]);
        assert_eq!(output, -2);
    }

    #[test]
    fn encode_u16_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode(1200_u16);
        assert_eq!(output, [1200]);
    }

    #[test]
    fn encode_u16_from_u32_array() {
        let output = u16::decode([1200_u32]);
        assert_eq!(output, 1200);
    }

    #[test]
    fn encode_i16_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode(-2_i16);
        assert_eq!(output, [65534]);
    }

    #[test]
    fn encode_i16_from_u32_array() {
        let output = i16::decode([65534_u32]);
        assert_eq!(output, -2);
    }

    #[test]
    fn encode_u32_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode(120341200_u32);
        assert_eq!(output, [120341200]);
    }

    #[test]
    fn encode_u32_from_u32_array() {
        let output = u32::decode([120341200_u32]);
        assert_eq!(output, 120341200);
    }

    #[test]
    fn encode_i32_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode(-2_i32);
        assert_eq!(output, [u32::MAX - 1]);
    }

    #[test]
    fn encode_char_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode('ç');
        assert_eq!(output, [231]);
    }

    #[test]
    fn encode_char_from_u32_array() {
        let output = char::decode([231_u32]);
        assert_eq!(output, 'ç');
    }

    #[test]
    fn encode_char_from_u32_array_replacement() {
        let output = char::decode([u32::MAX]);
        assert_eq!(output, char::REPLACEMENT_CHARACTER);
    }

    #[test]
    fn encode_i32_from_u32_array() {
        let output = i32::decode([u32::MAX - 1]);
        assert_eq!(output, -2);
    }

    #[test]
    fn encode_f32_from_u32_array() {
        let output = f32::decode([0xbfab_cdef_u32]);
        assert_eq!(output, -1.3422221_f32);
    }

    #[test]
    fn encode_f32_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode(-1.3422221_f32);
        assert_eq!(output, [0xbfab_cdef_u32]);
    }

    #[test]
    fn encode_u64_into_u32_array() {
        let output =
            TransformInto::<[u32; 2]>::encode(0x1234_5678_90ab_cdef_u64);
        assert_eq!(output, [0x90ab_cdef, 0x1234_5678]);
    }

    #[test]
    fn encode_u64_from_u32_array() {
        let output = u64::decode([0x90ab_cdef_u32, 0x1234_5678]);
        assert_eq!(output, 0x1234_5678_90ab_cdef);
    }

    #[test]
    fn encode_i64_into_u32_array() {
        let output = TransformInto::<[u32; 2]>::encode(-2_i64);
        assert_eq!(output, [0xffff_fffe, 0xffff_ffff]);
    }

    #[test]
    fn encode_i64_from_u32_array() {
        let output = i64::decode([0xffff_fffe, 0xffff_ffff_u32]);
        assert_eq!(output, -2);
    }

    #[test]
    fn encode_f64_from_u32_array() {
        let output = f64::decode([0x90ab_cdef_u32, 0xbff4_5678]);
        assert_eq!(output, -1.2711110736098552);
    }

    #[test]
    fn encode_f64_into_u32_array() {
        let output = TransformInto::<[u32; 2]>::encode(-1.2711110736098552_f64);
        assert_eq!(output, [0x90ab_cdef, 0xbff4_5678]);
    }

    #[test]
    fn encode_u128_into_u32_array() {
        let output = TransformInto::<[u32; 4]>::encode(
            0x1234_5678_90ab_cdef_fedc_ba90_8765_4321_u128,
        );
        assert_eq!(
            output,
            [0x8765_4321, 0xfedc_ba90, 0x90ab_cdef, 0x1234_5678]
        );
    }

    #[test]
    fn encode_u128_from_u32_array() {
        let output = u128::decode([
            0x8765_4321,
            0xfedc_ba90_u32,
            0x90ab_cdef,
            0x1234_5678,
        ]);
        assert_eq!(output, 0x1234_5678_90ab_cdef_fedc_ba90_8765_4321);
    }

    #[test]
    fn encode_i128_into_u32_array() {
        let output = TransformInto::<[u32; 4]>::encode(
            0x1234_5678_90ab_cdef_fedc_ba90_8765_4321_i128,
        );
        assert_eq!(
            output,
            [0x8765_4321, 0xfedc_ba90, 0x90ab_cdef, 0x1234_5678]
        );
    }

    #[test]
    fn encode_i128_from_u32_array() {
        let output = i128::decode([
            0x8765_4321,
            0xfedc_ba90_u32,
            0x90ab_cdef,
            0x1234_5678,
        ]);
        assert_eq!(output, 0x1234_5678_90ab_cdef_fedc_ba90_8765_4321);
    }

    #[test]
    fn encode_u8_2_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode([0xcd_u8, 0x12]);
        assert_eq!(output, [0x12cd]);
    }

    #[test]
    fn encode_u8_2_from_u32_array() {
        let output = <[u8; 2]>::decode([0x12cd]);
        assert_eq!(output, [0xcd_u8, 0x12]);
    }

    #[test]
    fn encode_u8_3_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode([0xcd_u8, 0x12, 0x34]);
        assert_eq!(output, [0x3412cd]);
    }

    #[test]
    fn encode_u8_3_from_u32_array() {
        let output = <[u8; 3]>::decode([0x3412cd]);
        assert_eq!(output, [0xcd_u8, 0x12, 0x34]);
    }

    #[test]
    fn encode_u8_4_into_u32_array() {
        let output =
            TransformInto::<[u32; 1]>::encode([0xcd_u8, 0x12, 0x34, 0x9a]);
        assert_eq!(output, [0x9a3412cd]);
    }

    #[test]
    fn encode_u8_4_from_u32_array() {
        let output = <[u8; 4]>::decode([0x9a3412cd]);
        assert_eq!(output, [0xcd_u8, 0x12, 0x34, 0x9a]);
    }

    #[test]
    fn encode_i8_2_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode([-2i8, 0x12]);
        assert_eq!(output, [0x12fe]);
    }

    #[test]
    fn encode_i8_2_from_u32_array() {
        let output = <[i8; 2]>::decode([0x12fe]);
        assert_eq!(output, [-2_i8, 0x12]);
    }

    #[test]
    fn encode_i8_3_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode([-2_i8, 0x12, 0x34]);
        assert_eq!(output, [0x3412fe]);
    }

    #[test]
    fn encode_i8_3_from_u32_array() {
        let output = <[i8; 3]>::decode([0x3412fe]);
        assert_eq!(output, [-2_i8, 0x12, 0x34]);
    }

    #[test]
    fn encode_i8_4_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode([-2_i8, 0x12, 0x34, -3]);
        assert_eq!(output, [0xfd3412fe]);
    }

    #[test]
    fn encode_i8_4_from_u32_array() {
        let output = <[i8; 4]>::decode([0xfd3412fe]);
        assert_eq!(output, [-2_i8, 0x12, 0x34, -3]);
    }

    #[test]
    fn encode_u16_2_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode([0xcdfe_u16, 0x1234]);
        assert_eq!(output, [0x1234_cdfe]);
    }

    #[test]
    fn encode_u16_2_from_u32_array() {
        let output = <[u16; 2]>::decode([0x1234_cdfe_u32]);
        assert_eq!(output, [0xcdfe_u16, 0x1234]);
    }

    #[test]
    fn encode_i16_2_into_u32_array() {
        let output = TransformInto::<[u32; 1]>::encode([-2_i16, 0x1234]);
        assert_eq!(output, [0x1234_fffe]);
    }

    #[test]
    fn encode_i16_2_from_u32_array() {
        let output = <[i16; 2]>::decode([0x1234_fffe_u32]);
        assert_eq!(output, [-2_i16, 0x1234]);
    }

    #[test]
    #[cfg(feature = "uuid")]
    fn encode_uuid_from_u32_array() {
        let output = uuid::Uuid::decode([
            0x8765_4321,
            0xfedc_ba90_u32,
            0x90ab_cdef,
            0x1234_5678,
        ]);
        assert_eq!(output.as_u128(), 0x1234_5678_90ab_cdef_fedc_ba90_8765_4321);
    }

    #[test]
    #[cfg(feature = "uuid")]
    fn encode_uuid_into_u32_array() {
        let output = TransformInto::<[u32; 4]>::encode(
            0x1234_5678_90ab_cdef_fedc_ba90_8765_4321_i128,
        );
        assert_eq!(
            output,
            [0x8765_4321, 0xfedc_ba90, 0x90ab_cdef, 0x1234_5678]
        );
    }
}
