//! When serializing or deserializing DAG-CBOR goes wrong.

use alloc::{
    collections::TryReserveError,
    string::{String, ToString},
};
use core::{convert::Infallible, fmt};

pub use cbor4ii::core::error::{ArithmeticOverflow, Len};
use serde::{de, ser};

/// An encoding error.
#[derive(Debug)]
pub enum EncodeError<E> {
    /// Custom error message.
    Msg(String),
    /// IO Error.
    Write(E),
}

impl<E> From<E> for EncodeError<E> {
    fn from(err: E) -> EncodeError<E> {
        EncodeError::Write(err)
    }
}

#[cfg(feature = "std")]
impl<E: std::error::Error + 'static> ser::Error for EncodeError<E> {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        EncodeError::Msg(msg.to_string())
    }
}

#[cfg(not(feature = "std"))]
impl<E: fmt::Debug> ser::Error for EncodeError<E> {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        EncodeError::Msg(msg.to_string())
    }
}

#[cfg(feature = "std")]
impl<E: std::error::Error + 'static> std::error::Error for EncodeError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EncodeError::Msg(_) => None,
            EncodeError::Write(err) => Some(err),
        }
    }
}

#[cfg(not(feature = "std"))]
impl<E: fmt::Debug> ser::StdError for EncodeError<E> {}

impl<E: fmt::Debug> fmt::Display for EncodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl<E: fmt::Debug> From<cbor4ii::core::error::EncodeError<E>> for EncodeError<E> {
    fn from(err: cbor4ii::core::error::EncodeError<E>) -> EncodeError<E> {
        match err {
            cbor4ii::core::error::EncodeError::Write(e) => EncodeError::Write(e),
            // Future-proof against new upstream variants without an SDK bump; loses structured info
            // but preserves the Display string.
            _ => EncodeError::Msg(err.to_string()),
        }
    }
}

/// A decoding error.
#[derive(Debug)]
pub enum DecodeError<E> {
    /// Custom error message.
    Msg(String),
    /// IO error.
    Read(E),
    /// End of file.
    Eof {
        /// Type name.
        name: &'static str,
        /// Expected length.
        expect: Len,
    },
    /// Unexpected byte.
    Mismatch {
        /// Type name.
        name: &'static str,
        /// Unexpected byte.
        found: u8,
    },
    /// Unsupported byte.
    Unsupported {
        /// Type name.
        name: &'static str,
        /// Unsupported byte.
        found: u8,
    },
    /// Length wasn't large enough.
    RequireLength {
        /// Type name.
        name: &'static str,
        /// Available length.
        found: Len,
    },
    /// Required a borrow.
    RequireBorrowed {
        /// Type name.
        name: &'static str,
    },
    /// Invalid UTF-8.
    RequireUtf8 {
        /// Type name.
        name: &'static str,
    },
    /// Length overflow.
    LengthOverflow {
        /// Type name.
        name: &'static str,
        /// Encoded length.
        found: Len,
    },
    /// Cast overflow.
    CastOverflow {
        /// Type name.
        name: &'static str,
    },
    /// Arithmetic overflow.
    ArithmeticOverflow {
        /// Type name.
        name: &'static str,
        /// Direction of the overflow.
        ty: ArithmeticOverflow,
    },
    /// Recursion limit reached.
    DepthOverflow {
        /// Type name.
        name: &'static str,
    },
    /// CBOR array/map length didn't match what serde expected.
    LengthMismatch {
        /// Type name.
        name: &'static str,
        /// Expected length.
        expect: usize,
        /// Actual length.
        value: usize,
    },
    /// Trailing data.
    TrailingData,
    /// Indefinite sized item was encountered.
    IndefiniteSize,
    /// An integer or length was not minimally encoded.
    NonMinimal {
        /// Type name.
        name: &'static str,
        /// The non-minimal head byte.
        found: u8,
    },
    /// Duplicate keys are not allowed in Maps.
    DuplicateKey,
    /// Map keys were not sorted in the order DAG-CBOR requires (shorter keys first, then
    /// bytewise).
    UnorderedKey,
}

impl<E> From<E> for DecodeError<E> {
    fn from(err: E) -> DecodeError<E> {
        DecodeError::Read(err)
    }
}

#[cfg(feature = "std")]
impl<E: std::error::Error + 'static> de::Error for DecodeError<E> {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DecodeError::Msg(msg.to_string())
    }
}

#[cfg(not(feature = "std"))]
impl<E: fmt::Debug> de::Error for DecodeError<E> {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        DecodeError::Msg(msg.to_string())
    }
}

#[cfg(feature = "std")]
impl<E: std::error::Error + 'static> std::error::Error for DecodeError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DecodeError::Read(err) => Some(err),
            _ => None,
        }
    }
}

#[cfg(not(feature = "std"))]
impl<E: fmt::Debug> ser::StdError for DecodeError<E> {}

impl<E: fmt::Debug> fmt::Display for DecodeError<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl<E: fmt::Debug> From<cbor4ii::core::error::DecodeError<E>> for DecodeError<E> {
    fn from(err: cbor4ii::core::error::DecodeError<E>) -> DecodeError<E> {
        use cbor4ii::core::error::DecodeError as Cbor4iiError;
        match err {
            Cbor4iiError::Read(read) => DecodeError::Read(read),
            Cbor4iiError::Eof { name, expect } => DecodeError::Eof { name, expect },
            Cbor4iiError::Mismatch { name, found } => DecodeError::Mismatch { name, found },
            Cbor4iiError::Unsupported { name, found } => DecodeError::Unsupported { name, found },
            Cbor4iiError::RequireLength { name, found } => {
                DecodeError::RequireLength { name, found }
            }
            Cbor4iiError::RequireBorrowed { name } => DecodeError::RequireBorrowed { name },
            Cbor4iiError::RequireUtf8 { name } => DecodeError::RequireUtf8 { name },
            Cbor4iiError::LengthOverflow { name, found } => {
                DecodeError::LengthOverflow { name, found }
            }
            Cbor4iiError::CastOverflow { name } => DecodeError::CastOverflow { name },
            Cbor4iiError::ArithmeticOverflow { name, ty } => {
                DecodeError::ArithmeticOverflow { name, ty }
            }
            Cbor4iiError::DepthOverflow { name } => DecodeError::DepthOverflow { name },
            // Future-proof against new upstream variants without an SDK bump; loses structured info
            // but preserves the Display string.
            _ => DecodeError::Msg(err.to_string()),
        }
    }
}

/// Encode and Decode error combined.
#[derive(Debug)]
pub enum CodecError {
    /// A decoding error.
    Decode(DecodeError<Infallible>),
    /// An encoding error.
    Encode(EncodeError<TryReserveError>),
    /// A decoding error.
    #[cfg(feature = "std")]
    DecodeIo(DecodeError<std::io::Error>),
    /// An encoding error.
    #[cfg(feature = "std")]
    EncodeIo(EncodeError<std::io::Error>),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(f, "decode error: {}", error),
            Self::Encode(error) => write!(f, "encode error: {}", error),
            #[cfg(feature = "std")]
            Self::DecodeIo(error) => write!(f, "decode io error: {}", error),
            #[cfg(feature = "std")]
            Self::EncodeIo(error) => write!(f, "encode io error: {}", error),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CodecError {}

impl From<DecodeError<Infallible>> for CodecError {
    fn from(error: DecodeError<Infallible>) -> Self {
        Self::Decode(error)
    }
}

#[cfg(feature = "std")]
impl From<DecodeError<std::io::Error>> for CodecError {
    fn from(error: DecodeError<std::io::Error>) -> Self {
        Self::DecodeIo(error)
    }
}

impl From<EncodeError<TryReserveError>> for CodecError {
    fn from(error: EncodeError<TryReserveError>) -> Self {
        Self::Encode(error)
    }
}

#[cfg(feature = "std")]
impl From<EncodeError<std::io::Error>> for CodecError {
    fn from(error: EncodeError<std::io::Error>) -> Self {
        Self::EncodeIo(error)
    }
}
