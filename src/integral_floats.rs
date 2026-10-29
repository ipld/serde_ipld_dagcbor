//! Conversions between integers and floats for the `_experimental-integral-floats` feature.
use core::marker::PhantomData;

use cbor4ii::core::dec;
use serde::de::{self, Visitor};

use crate::de::decode_minimal_i128;
use crate::error::DecodeError;

/// Deserializes an `f32`, rejecting values that aren't exactly representable.
///
/// A plain `f32` field already rejects them, but not when serde buffers the value (e.g. for
/// untagged enums or `#[serde(flatten)]`), as then serde converts it itself with `as`. Use this
/// function to keep the check in those cases. Integers are accepted if they are exactly
/// representable, and either serde buffers the value or [`Deserializer::integral_floats`] is
/// enabled. When serde buffers the value, integers below `i64::MIN` are rejected, as serde can't
/// buffer them.
///
/// [`Deserializer::integral_floats`]: crate::de::Deserializer::integral_floats
///
/// ```
/// # use serde_derive::Deserialize;
/// #[derive(Deserialize)]
/// #[serde(untagged)]
/// enum Value {
///     Float(#[serde(deserialize_with = "serde_ipld_dagcbor::de::lossless_f32")] f32),
///     String(String),
/// }
/// ```
///
/// Requires the `_experimental-integral-floats` feature, which may change or be removed in a
/// non-breaking release.
pub fn lossless_f32<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    deserializer.deserialize_f32(LosslessFloatVisitor(PhantomData))
}

/// Deserializes an `f64`, rejecting integers that aren't exactly representable.
///
/// See [`lossless_f32`] for details.
///
/// Requires the `_experimental-integral-floats` feature, which may change or be removed in a
/// non-breaking release.
pub fn lossless_f64<'de, D: de::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    deserializer.deserialize_f64(LosslessFloatVisitor(PhantomData))
}

pub(crate) trait LosslessFloat: Copy {
    const NAME: &'static str;
    fn from_f64(value: f64) -> Self;
    fn to_f64(self) -> f64;
    fn from_i128(value: i128) -> Self;
    fn to_i128(self) -> i128;

    /// Converts an integer, if it's exactly representable.
    fn from_i128_lossless<E: de::Error>(value: i128) -> Result<Self, E> {
        let float = Self::from_i128(value);
        if float.to_i128() != value {
            return Err(E::custom(format_args!(
                "Integer value has more precision than {} can represent, loss of precision",
                Self::NAME
            )));
        }
        Ok(float)
    }
}

impl LosslessFloat for f32 {
    const NAME: &'static str = "f32";
    fn from_f64(value: f64) -> Self {
        value as f32
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn from_i128(value: i128) -> Self {
        value as f32
    }
    fn to_i128(self) -> i128 {
        self as i128
    }
}

impl LosslessFloat for f64 {
    const NAME: &'static str = "f64";
    fn from_f64(value: f64) -> Self {
        value
    }
    fn to_f64(self) -> f64 {
        self
    }
    fn from_i128(value: i128) -> Self {
        value as f64
    }
    fn to_i128(self) -> i128 {
        self as i128
    }
}

struct LosslessFloatVisitor<F>(PhantomData<F>);

impl<F: LosslessFloat> Visitor<'_> for LosslessFloatVisitor<F> {
    type Value = F;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(formatter, "a number exactly representable as {}", F::NAME)
    }

    fn visit_f32<E: de::Error>(self, value: f32) -> Result<F, E> {
        Ok(F::from_f64(value as f64))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<F, E> {
        let float = F::from_f64(value);
        if float.to_f64().to_bits() != value.to_bits() {
            return Err(E::custom(format_args!(
                "Float value has more precision than {} can represent, loss of precision",
                F::NAME
            )));
        }
        Ok(float)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<F, E> {
        self.visit_i128(value.into())
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<F, E> {
        self.visit_i128(value.into())
    }

    fn visit_i128<E: de::Error>(self, value: i128) -> Result<F, E> {
        F::from_i128_lossless(value)
    }
}

/// Decodes an integer where a float is expected, if it's exactly representable as that float.
///
/// Some encoders write floats with an integral value as integers.
pub(crate) fn decode_int_as_float<'de, R: dec::Read<'de>, F: LosslessFloat>(
    name: &'static str,
    head: u8,
    reader: &mut R,
) -> Result<F, DecodeError<R::Error>>
where
    DecodeError<R::Error>: de::Error,
{
    let value = decode_minimal_i128(name, head, reader)?;
    F::from_i128_lossless(value)
}
