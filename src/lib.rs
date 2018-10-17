//! Implementation of the [ctlv format](https://github.com/AljoschaMeyer/ctlv) in rust.

extern crate varu64;

use varu64::DecodeError as VarU64Error;

use std::{fmt, error};

// TODO enforce match between type and length, or leave it unsafe?
// TODO test

/// Everything that can go wrong when decoding a ctlv.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecodeError {
    /// Decoding the type failed with the wrapped error.
    Type(VarU64Error),
    /// Decoding the length failed with the wrapped error.
    Length(VarU64Error),
    /// The slice contains less data than the encoding needs.
    ///
    /// This is only used in these three cases:
    ///
    /// - the input is the empty slice
    /// - the input ended after the type
    /// - not enough data for the `value` is available
    ///
    /// End of input inside the `type` or `length` varu64 is signaled via the
    /// `Type` and `Length` variants respectively.
    UnexpectedEndOfInput,
}
use self::DecodeError::*;

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        match self {
            Type(e) => write!(f, "Invalid ctlv type: {}", e),
            Length(e) => write!(f, "Invalid ctlv length: {}", e),
            UnexpectedEndOfInput => write!(f, "Invalid ctlv: Not enough input bytes"),
        }
    }
}

impl error::Error for DecodeError {}

/// A type-length-value triple that owns its value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ctlv {
    /// The type of the triple.
    pub type_: u64,
    /// The value, from which the length can be derived.
    pub value: Vec<u8>,
}

impl Ctlv {
    /// Return how many bytes the encoding of the `Ctlv` will take up.
    pub fn encoding_length(&self) -> usize {
        self.as_ctlv_ref().encoding_length()
    }

    /// Encodes this `Ctlv` into the output buffer, returning how many bytes have been written.
    ///
    /// # Panics
    /// Panics if the buffer is not large enough to hold the encoding.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        self.as_ctlv_ref().encode(out)
    }

    /// Decode a `Ctlv` from the input buffer, returning it and how many bytes were read.
    pub fn decode(input: &[u8]) -> Result<(Ctlv, usize), (DecodeError, usize)> {
        let (tmp, total_len) = CtlvRef::decode(input)?;
        let mut value = Vec::with_capacity(tmp.value.len());
        value.extend_from_slice(&tmp.value);

        Ok((Ctlv {
                type_: tmp.type_,
                value,
            },
            total_len))
    }

    /// Returns a `CtlvRef` that borrows its value from this `Ctlv`.
    pub fn as_ctlv_ref(&self) -> CtlvRef {
        CtlvRef {
            type_: self.type_,
            value: &self.value,
        }
    }

    /// Returns a `CtlvRefMut` that mutably borrows its value from this `Ctlv`.
    pub fn as_ctlv_ref_mut(&mut self) -> CtlvRefMut {
        CtlvRefMut {
            type_: self.type_,
            value: &mut self.value,
        }
    }
}

/// A type-length-value triple that immutably borrows its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CtlvRef<'a> {
    /// The type of the triple.
    pub type_: u64,
    /// The borrowed value.
    pub value: &'a [u8],
}

impl<'a> CtlvRef<'a> {
    /// Return how many bytes the encoding of the `CtlvRef` will take up.
    pub fn encoding_length(&self) -> usize {
        let length = self.value.len();
        let length_len = if length < 128 {
            0
        } else {
            varu64::encoding_length(length as u64)
        };

        return varu64::encoding_length(self.type_) + length_len + length;
    }

    /// Encodes this `CtlvRef` into the output buffer, returning how many bytes have been written.
    ///
    /// # Panics
    /// Panics if the buffer is not large enough to hold the encoding.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        let mut total = varu64::encode(self.type_, out);
        let length: usize = self.value.len();

        if length >= 128 {
            total += varu64::encode(length as u64, &mut out[total..]);
        }

        &mut out[total..total + length].copy_from_slice(self.value);

        return total + length;
    }

    /// Decode a `CtlvRef` from the input buffer, returning it and how many bytes were read.
    pub fn decode(input: &'a [u8]) -> Result<(CtlvRef<'a>, usize), (DecodeError, usize)> {
        let type_: u64;
        let length: usize;
        let total_len: usize;

        match varu64::decode(input) {
            Err((e, l)) => return Err((Type(e), l)),
            Ok((t @ 0...127, l)) => {
                type_ = t;
                length = 1 << (type_ >> 3);
                total_len = l;
            }
            Ok((t, l)) => {
                type_ = t;

                match varu64::decode(&input[l..]) {
                    Err((e, l2)) => return Err((Length(e), l + l2)),
                    Ok((len, l2)) => {
                        length = len as usize;
                        total_len = l + l2;
                    }
                }
            }
        }

        let data = &input[total_len..];
        if data.len() < length {
            return Err((UnexpectedEndOfInput, input.len()));
        } else {
            return Ok((CtlvRef {
                           type_,
                           value: &data[..length],
                       },
                       total_len + length));
        }
    }
}

/// A type-length-value triple that mutably borrows its value.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CtlvRefMut<'a> {
    /// The type of the triple.
    pub type_: u64,
    /// The mutably borrowed value.
    pub value: &'a mut [u8],
}

impl<'a> CtlvRefMut<'a> {
    /// Return how many bytes the encoding of the `Ctlv` will take up.
    pub fn encoding_length(&self) -> usize {
        self.as_ctlv_ref().encoding_length()
    }

    /// Encodes this `CtlvRefMut` into the output buffer, returning how many bytes have been written.
    ///
    /// # Panics
    /// Panics if the buffer is not large enough to hold the encoding.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        self.as_ctlv_ref().encode(out)
    }

    /// Decode a `CtlvRefMut` from the input buffer, returning it and how many bytes were read.
    pub fn decode(input: &'a mut [u8]) -> Result<(CtlvRefMut<'a>, usize), (DecodeError, usize)> {
        let (tmp, total_len) = CtlvRef::decode(input)?;
        let tmp_type = tmp.type_;

        let start = total_len - tmp.value.len();
        let value = &mut input[start..total_len];

        Ok((CtlvRefMut {
                type_: tmp_type,
                value,
            },
            total_len))
    }

    /// Returns a `CtlvRef` that borrows the same value as this `CtlvRefMut`.
    pub fn as_ctlv_ref(&self) -> CtlvRef {
        CtlvRef {
            type_: self.type_,
            value: &self.value,
        }
    }
}
