//! Serialize a Rust data structure to CBOR data.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::convert::TryFrom;

#[cfg(feature = "std")]
pub use crate::write::IoWrite;
pub use crate::write::{SliceWrite, Write};

use crate::error::{Error, Result};
use crate::CBOR_TAGS_CID;
use cid::serde::CID_SERDE_PRIVATE_IDENTIFIER;
use serde::ser::{self, Serialize, SerializeMap};
#[cfg(feature = "std")]
use std::io;

const MAJOR_TYPE_BYTES: u8 = 2;
const MAJOR_TYPE_TAG: u8 = 6;

/// Serializes a value to a vector.
#[cfg(any(feature = "std", feature = "alloc"))]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
    T: ser::Serialize,
{
    let mut vec = Vec::new();
    value.serialize(&mut Serializer::new(&mut vec))?;
    Ok(vec)
}

/// Serializes a value to a vector in packed format.
#[cfg(feature = "std")]
pub fn to_vec_packed<T>(value: &T) -> Result<Vec<u8>>
where
    T: ser::Serialize,
{
    let mut vec = Vec::new();
    value.serialize(&mut Serializer::new(&mut IoWrite::new(&mut vec)).packed_format())?;
    Ok(vec)
}

/// Serializes a value to a writer.
#[cfg(feature = "std")]
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ser::Serialize,
{
    value.serialize(&mut Serializer::new(&mut IoWrite::new(writer)))
}

/// A structure for serializing Rust values to CBOR.
#[derive(Debug)]
pub struct Serializer<W> {
    writer: W,
    packed: bool,
    enum_as_map: bool,
}

impl<W> Serializer<W>
where
    W: Write,
{
    /// Creates a new CBOR serializer.
    ///
    /// `to_vec` and `to_writer` should normally be used instead of this method.
    #[inline]
    pub fn new(writer: W) -> Self {
        Serializer {
            writer,
            packed: false,
            enum_as_map: true,
        }
    }

    /// Choose concise/packed format for serializer.
    ///
    /// In the packed format enum variant names and field names
    /// are replaced with numeric indizes to conserve space.
    pub fn packed_format(mut self) -> Self {
        self.packed = true;
        self
    }

    /// Enable old enum format used by `serde_cbor` versions <= v0.9.
    ///
    /// The `legacy_enums` option determines how enums are encoded.
    ///
    /// This makes no difference when encoding and decoding enums using
    /// this crate, but it shows up when decoding to a `Value` or decoding
    /// in other languages.
    ///
    /// # Examples
    ///
    /// Given the following enum
    ///
    /// ```rust
    /// enum Enum {
    ///     Unit,
    ///     NewType(i32),
    ///     Tuple(String, bool),
    ///     Struct{ x: i32, y: i32 },
    /// }
    /// ```
    /// we will give the `Value` with the same encoding for each case using
    /// JSON notation.
    ///
    /// ## Default encodings
    ///
    /// * `Enum::Unit` encodes as `"Unit"`
    /// * `Enum::NewType(10)` encodes as `{"NewType": 10}`
    /// * `Enum::Tuple("x", true)` encodes as `{"Tuple": ["x", true]}`
    ///
    /// ## Legacy encodings
    ///
    /// * `Enum::Unit` encodes as `"Unit"`
    /// * `Enum::NewType(10)` encodes as `["NewType", 10]`
    /// * `Enum::Tuple("x", true)` encodes as `["Tuple", "x", true]`
    /// * `Enum::Struct{ x: 5, y: -5 }` encodes as `["Struct", {"x": 5, "y": -5}]`
    pub fn legacy_enums(mut self) -> Self {
        self.enum_as_map = false;
        self
    }

    /// Writes a CBOR self-describe tag to the stream.
    ///
    /// Tagging allows a decoder to distinguish different file formats based on their content
    /// without further information.
    #[inline]
    pub fn self_describe(&mut self) -> Result<()> {
        let mut buf = [6 << 5 | 25, 0, 0];
        (&mut buf[1..]).copy_from_slice(&55799u16.to_be_bytes());
        self.writer.write_all(&buf).map_err(|e| e.into())
    }

    /// Unwrap the `Writer` from the `Serializer`.
    #[inline]
    pub fn into_inner(self) -> W {
        self.writer
    }

    #[inline]
    fn write_u8(&mut self, major: u8, value: u8) -> Result<()> {
        if value <= 0x17 {
            self.writer.write_all(&[major << 5 | value])
        } else {
            let buf = [major << 5 | 24, value];
            self.writer.write_all(&buf)
        }
        .map_err(|e| e.into())
    }

    #[inline]
    fn write_u16(&mut self, major: u8, value: u16) -> Result<()> {
        if value <= u16::from(u8::max_value()) {
            self.write_u8(major, value as u8)
        } else {
            let mut buf = [major << 5 | 25, 0, 0];
            (&mut buf[1..]).copy_from_slice(&value.to_be_bytes());
            self.writer.write_all(&buf).map_err(|e| e.into())
        }
    }

    #[inline]
    fn write_u32(&mut self, major: u8, value: u32) -> Result<()> {
        if value <= u32::from(u16::max_value()) {
            self.write_u16(major, value as u16)
        } else {
            let mut buf = [major << 5 | 26, 0, 0, 0, 0];
            (&mut buf[1..]).copy_from_slice(&value.to_be_bytes());
            self.writer.write_all(&buf).map_err(|e| e.into())
        }
    }

    #[inline]
    fn write_u64(&mut self, major: u8, value: u64) -> Result<()> {
        if value <= u64::from(u32::max_value()) {
            self.write_u32(major, value as u32)
        } else {
            let mut buf = [major << 5 | 27, 0, 0, 0, 0, 0, 0, 0, 0];
            (&mut buf[1..]).copy_from_slice(&value.to_be_bytes());
            self.writer.write_all(&buf).map_err(|e| e.into())
        }
    }

    #[inline]
    fn serialize_collection(
        &mut self,
        major: u8,
        len: Option<usize>,
    ) -> Result<CollectionSerializer<'_, W>> {
        let needs_eof = match len {
            Some(len) => {
                self.write_u64(major, len as u64)?;
                false
            }
            None => {
                self.writer
                    .write_all(&[major << 5 | 31])
                    .map_err(|e| e.into())?;
                true
            }
        };

        Ok(CollectionSerializer {
            ser: self,
            needs_eof,
        })
    }
}

impl<'a, W> ser::Serializer for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = CollectionSerializer<'a, W>;
    type SerializeTuple = &'a mut Serializer<W>;
    type SerializeTupleStruct = &'a mut Serializer<W>;
    type SerializeTupleVariant = &'a mut Serializer<W>;
    type SerializeMap = CollectionSerializer<'a, W>;
    type SerializeStruct = StructSerializer<'a, W>;
    type SerializeStructVariant = StructSerializer<'a, W>;

    #[inline]
    fn serialize_bool(self, value: bool) -> Result<()> {
        let value = if value { 0xf5 } else { 0xf4 };
        self.writer.write_all(&[value]).map_err(|e| e.into())
    }

    #[inline]
    fn serialize_i8(self, value: i8) -> Result<()> {
        if value < 0 {
            self.write_u8(1, -(value + 1) as u8)
        } else {
            self.write_u8(0, value as u8)
        }
    }

    #[inline]
    fn serialize_i16(self, value: i16) -> Result<()> {
        if value < 0 {
            self.write_u16(1, -(value + 1) as u16)
        } else {
            self.write_u16(0, value as u16)
        }
    }

    #[inline]
    fn serialize_i32(self, value: i32) -> Result<()> {
        if value < 0 {
            self.write_u32(1, -(value + 1) as u32)
        } else {
            self.write_u32(0, value as u32)
        }
    }

    #[inline]
    fn serialize_i64(self, value: i64) -> Result<()> {
        if value < 0 {
            self.write_u64(1, -(value + 1) as u64)
        } else {
            self.write_u64(0, value as u64)
        }
    }

    #[inline]
    fn serialize_i128(self, value: i128) -> Result<()> {
        if value < 0 {
            if -(value + 1) > i128::from(u64::max_value()) {
                return Err(Error::message("The number can't be stored in CBOR"));
            }
            self.write_u64(1, -(value + 1) as u64)
        } else {
            if value > i128::from(u64::max_value()) {
                return Err(Error::message("The number can't be stored in CBOR"));
            }
            self.write_u64(0, value as u64)
        }
    }

    #[inline]
    fn serialize_u8(self, value: u8) -> Result<()> {
        self.write_u8(0, value)
    }

    #[inline]
    fn serialize_u16(self, value: u16) -> Result<()> {
        self.write_u16(0, value)
    }

    #[inline]
    fn serialize_u32(self, value: u32) -> Result<()> {
        self.write_u32(0, value)
    }

    #[inline]
    fn serialize_u64(self, value: u64) -> Result<()> {
        self.write_u64(0, value)
    }

    #[inline]
    fn serialize_u128(self, value: u128) -> Result<()> {
        if value > u128::from(u64::max_value()) {
            return Err(Error::message("The number can't be stored in CBOR"));
        }
        self.write_u64(0, value as u64)
    }

    #[inline]
    #[allow(clippy::float_cmp)]
    fn serialize_f32(self, value: f32) -> Result<()> {
        self.serialize_f64(value.into())
    }

    #[inline]
    #[allow(clippy::float_cmp)]
    fn serialize_f64(self, value: f64) -> Result<()> {
        if !value.is_finite() {
            Err(Error::message(
                "Float must be a finite number, not Infinity or NaN",
            ))
        } else {
            let mut buf = [0xfb, 0, 0, 0, 0, 0, 0, 0, 0];
            (&mut buf[1..]).copy_from_slice(&value.to_bits().to_be_bytes());
            self.writer.write_all(&buf).map_err(|e| e.into())
        }
    }

    #[inline]
    fn serialize_char(self, value: char) -> Result<()> {
        // A char encoded as UTF-8 takes 4 bytes at most.
        let mut buf = [0; 4];
        self.serialize_str(value.encode_utf8(&mut buf))
    }

    #[inline]
    fn serialize_str(self, value: &str) -> Result<()> {
        self.write_u64(3, value.len() as u64)?;
        self.writer
            .write_all(value.as_bytes())
            .map_err(|e| e.into())
    }

    #[inline]
    fn serialize_bytes(self, value: &[u8]) -> Result<()> {
        self.write_u64(2, value.len() as u64)?;
        self.writer.write_all(value).map_err(|e| e.into())
    }

    #[inline]
    fn serialize_unit(self) -> Result<()> {
        self.serialize_none()
    }

    #[inline]
    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(self)
    }

    #[inline]
    fn serialize_none(self) -> Result<()> {
        self.writer.write_all(&[0xf6]).map_err(|e| e.into())
    }

    #[inline]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        if self.packed {
            self.serialize_u32(variant_index)
        } else {
            self.serialize_str(variant)
        }
    }

    #[inline]
    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        if name == CID_SERDE_PRIVATE_IDENTIFIER {
            value.serialize(&mut CidSerializer(self))
        } else {
            value.serialize(self)
        }
    }

    #[inline]
    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        if self.enum_as_map {
            self.write_u64(5, 1u64)?;
            variant.serialize(&mut *self)?;
        } else {
            self.writer.write_all(&[4 << 5 | 2]).map_err(|e| e.into())?;
            self.serialize_unit_variant(name, variant_index, variant)?;
        }
        value.serialize(self)
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<CollectionSerializer<'a, W>> {
        self.serialize_collection(4, len)
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<&'a mut Serializer<W>> {
        self.write_u64(4, len as u64)?;
        Ok(self)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<&'a mut Serializer<W>> {
        self.serialize_tuple(len)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<&'a mut Serializer<W>> {
        if self.enum_as_map {
            self.write_u64(5, 1u64)?;
            variant.serialize(&mut *self)?;
            self.serialize_tuple(len)
        } else {
            self.write_u64(4, (len + 1) as u64)?;
            self.serialize_unit_variant(name, variant_index, variant)?;
            Ok(self)
        }
    }

    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<CollectionSerializer<'a, W>> {
        self.serialize_collection(5, len)
    }

    #[cfg(not(feature = "std"))]
    fn collect_str<T: ?Sized>(self, value: &T) -> Result<()>
    where
        T: core::fmt::Display,
    {
        use crate::write::FmtWrite;
        use core::fmt::Write;

        let mut w = FmtWrite::new(&mut self.writer);
        write!(w, "{}", value)?;
        Ok(())
    }

    #[inline]
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<StructSerializer<'a, W>> {
        self.write_u64(5, len as u64)?;
        Ok(StructSerializer { ser: self, idx: 0 })
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<StructSerializer<'a, W>> {
        if self.enum_as_map {
            self.write_u64(5, 1u64)?;
        } else {
            self.writer.write_all(&[4 << 5 | 2]).map_err(|e| e.into())?;
        }
        self.serialize_unit_variant(name, variant_index, variant)?;
        self.serialize_struct(name, len)
    }

    #[inline]
    fn is_human_readable(&self) -> bool {
        false
    }

    fn collect_map<K, V, I>(self, iter: I) -> Result<()>
    where
        K: Serialize,
        V: Serialize,
        I: IntoIterator<Item = (K, V)>,
    {
        // CBOR RFC-7049 specifies a canonical sort order, where keys are sorted by length first.
        // This was later revised with RFC-8949, but we need to stick to the original order to stay
        // compatible with existing data.
        // We first serialize each map entry into a buffer and then sort those buffers. Byte-wise
        // comparison gives us the right order as keys in DAG-CBOR are always strings and prefixed
        // with the length. Once sorted they are written to the actual output.
        let mut buffer: Vec<u8> = Vec::new();
        let mut mem_serializer = Serializer::new(&mut buffer);
        let mut serializer = CollectionSerializer {
            ser: &mut mem_serializer,
            needs_eof: false,
        };
        let mut entries = Vec::new();
        for (key, value) in iter {
            serializer.serialize_entry(&key, &value)?;
            entries.push(serializer.ser.writer.clone());
            serializer.ser.writer.clear();
        }

        self.write_u64(5, entries.len() as u64)?;
        entries.sort_unstable();
        for entry in entries {
            self.writer.write_all(&entry).map_err(Into::into)?;
        }

        Ok(())
    }
}

impl<'a, W> ser::SerializeTuple for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeTupleStruct for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeTupleVariant for &'a mut Serializer<W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<()> {
        Ok(())
    }
}

#[doc(hidden)]
pub struct StructSerializer<'a, W> {
    ser: &'a mut Serializer<W>,
    idx: u32,
}

impl<'a, W> StructSerializer<'a, W>
where
    W: Write,
{
    #[inline]
    fn serialize_field_inner<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        if self.ser.packed {
            self.idx.serialize(&mut *self.ser)?;
        } else {
            key.serialize(&mut *self.ser)?;
        }
        value.serialize(&mut *self.ser)?;
        self.idx += 1;
        Ok(())
    }

    #[inline]
    fn skip_field_inner(&mut self, _: &'static str) -> Result<()> {
        self.idx += 1;
        Ok(())
    }

    #[inline]
    fn end_inner(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeStruct for StructSerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.serialize_field_inner(key, value)
    }

    #[inline]
    fn skip_field(&mut self, key: &'static str) -> Result<()> {
        self.skip_field_inner(key)
    }

    #[inline]
    fn end(self) -> Result<()> {
        self.end_inner()
    }
}

impl<'a, W> ser::SerializeStructVariant for StructSerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        self.serialize_field_inner(key, value)
    }

    #[inline]
    fn skip_field(&mut self, key: &'static str) -> Result<()> {
        self.skip_field_inner(key)
    }

    #[inline]
    fn end(self) -> Result<()> {
        self.end_inner()
    }
}

#[doc(hidden)]
pub struct CollectionSerializer<'a, W> {
    ser: &'a mut Serializer<W>,
    needs_eof: bool,
}

impl<'a, W> CollectionSerializer<'a, W>
where
    W: Write,
{
    #[inline]
    fn end_inner(self) -> Result<()> {
        if self.needs_eof {
            self.ser.writer.write_all(&[0xff]).map_err(|e| e.into())
        } else {
            Ok(())
        }
    }
}

impl<'a, W> ser::SerializeSeq for CollectionSerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(&mut *self.ser)
    }

    #[inline]
    fn end(self) -> Result<()> {
        self.end_inner()
    }
}

impl<'a, W> ser::SerializeMap for CollectionSerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        key.serialize(&mut *self.ser)
    }

    #[inline]
    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + ser::Serialize,
    {
        value.serialize(&mut *self.ser)
    }

    #[inline]
    fn end(self) -> Result<()> {
        self.end_inner()
    }
}

/// Serializing a CID correctly as DAG-CBOR.
struct CidSerializer<'a, W>(&'a mut Serializer<W>);

impl<'a, W> ser::Serializer for &'a mut CidSerializer<'a, W>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_i8(self, _value: i8) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_i16(self, _value: i16) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_i32(self, _value: i32) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_i64(self, _value: i64) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_u8(self, _value: u8) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_u16(self, _value: u16) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_u32(self, _value: u32) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_u64(self, _value: u64) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_f32(self, _value: f32) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_f64(self, _value: f64) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_char(self, _value: char) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_str(self, _value: &str) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok> {
        // CIDs are serialized with CBOR tag 42.
        self.0.write_u8(MAJOR_TYPE_TAG, CBOR_TAGS_CID)?;
        // THe CID is prefixed with a null byte, hence add 1 to the length of the byte string.
        self.0.write_u64(
            MAJOR_TYPE_BYTES,
            u64::try_from(value.len() + 1).expect("Platform must be at most 64-bit"),
        )?;
        // The bytes of the CID is prefixed with a null byte when encoded as CBOR.
        self.0.writer.write_all(&[0x00]).map_err(Into::into)?;
        self.0.writer.write_all(value).map_err(Into::into)?;
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_unit(self) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_unit_struct(self, _name: &str) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_unit_variant(
        self,
        _name: &str,
        _variant_index: u32,
        _variant: &str,
    ) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &str,
        _value: &T,
    ) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &str,
        _variant_index: u32,
        _variant: &str,
        _value: &T,
    ) -> Result<Self::Ok> {
        Err(Error::message("unreachable"))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::message("unreachable"))
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::message("unreachable"))
    }
    fn serialize_tuple_struct(
        self,
        _name: &str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::message("unreachable"))
    }
    fn serialize_tuple_variant(
        self,
        _name: &str,
        _variant_index: u32,
        _variant: &str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::message("unreachable"))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::message("unreachable"))
    }
    fn serialize_struct(self, _name: &str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(Error::message("unreachable"))
    }
    fn serialize_struct_variant(
        self,
        _name: &str,
        _variant_index: u32,
        _variant: &str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::message("unreachable"))
    }
}
