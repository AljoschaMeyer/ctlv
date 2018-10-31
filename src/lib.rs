//! Implementation of the [ctlv format](https://github.com/AljoschaMeyer/ctlv) in rust.
//!
//! None of the structs enforce type-implied lengths upon serialization. It is up to the
//! user to ensure that ctlvs with a type below 128 contain data of the correct length.

extern crate varu64;

use varu64::DecodeError as VarU64Error;

use std::{fmt, error, io};

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

    /// Encodes this `Ctlv` into the writer, returning how many bytes have been written.
    pub fn encode_write<W: io::Write>(&self, w: W) -> Result<usize, io::Error> {
        self.as_ctlv_ref().encode_write(w)
    }

    /// Encodes this `Ctlv` as an owned `Vec<u8>`.
    pub fn encode_vec(&self) -> Vec<u8> {
        self.as_ctlv_ref().encode_vec()
    }

    /// Encodes this `Ctlv` as an owned `String`.
    pub fn encode_string(&self) -> String {
        self.as_ctlv_ref().encode_string()
    }

    /// Decode a `Ctlv` from the input buffer, returning it and the remaining input.
    pub fn decode(input: &[u8]) -> Result<(Ctlv, &[u8]), (DecodeError, &[u8])> {
        let (tmp, tail) = CtlvRef::decode(input)?;
        let mut value = Vec::with_capacity(tmp.value.len());
        value.extend_from_slice(&tmp.value);

        Ok((Ctlv {
                type_: tmp.type_,
                value,
            },
            tail))
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
        let length_len = if self.type_ < 128 {
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

        if self.type_ >= 128 {
            total += varu64::encode(length as u64, &mut out[total..]);
        }

        &mut out[total..total + length].copy_from_slice(self.value);

        return total + length;
    }

    /// Encodes this `CtlvRef` into the writer, returning how many bytes have been written.
    pub fn encode_write<W: io::Write>(&self, mut w: W) -> Result<usize, io::Error> {
        let mut total = varu64::encode_write(self.type_, &mut w)?;
        let length: usize = self.value.len();

        if self.type_ >= 128 {
            total += varu64::encode_write(length as u64, &mut w)?;
        }

        w.write_all(self.value)?;

        Ok(total + length)
    }

    /// Encodes this `CtlvRef` as an owned `Vec<u8>`.
    pub fn encode_vec(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.value.len());
        self.encode_write(&mut out).unwrap();
        out
    }

    /// Encodes this `CtlvRef` as an owned `String`.
    pub fn encode_string(&self) -> String {
        unsafe { String::from_utf8_unchecked(self.encode_vec()) }
    }

    /// Decode a `CtlvRef` from the input buffer, returning it and the remaining input.
    pub fn decode(input: &'a [u8]) -> Result<(CtlvRef<'a>, &'a [u8]), (DecodeError, &'a [u8])> {
        let type_: u64;
        let length: usize;
        let remaining: &'a [u8];

        match varu64::decode(input) {
            Err((_, tail)) if tail.len() == 0 => return Err((UnexpectedEndOfInput, input)),
            Err((e, tail)) => return Err((Type(e), tail)),
            Ok((t @ 0...127, tail)) => {
                type_ = t;
                length = 1 << (type_ >> 3);
                remaining = tail;
            }
            Ok((t, tail)) => {
                type_ = t;

                match varu64::decode(tail) {
                    Err((e, tail2)) => return Err((Length(e), tail2)),
                    Ok((len, tail2)) => {
                        length = len as usize;
                        remaining = tail2;
                    }
                }
            }
        }

        if remaining.len() < length {
            return Err((UnexpectedEndOfInput, remaining));
        } else {
            return Ok((CtlvRef {
                           type_,
                           value: &remaining[..length],
                       },
                       &remaining[length..]));
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

    /// Encodes this `CtlvRefMut` into the writer, returning how many bytes have been written.
    pub fn encode_write<W: io::Write>(&self, w: W) -> Result<usize, io::Error> {
        self.as_ctlv_ref().encode_write(w)
    }

    /// Encodes this `CtlvRefMut` as an owned `Vec<u8>`.
    pub fn encode_vec(&self) -> Vec<u8> {
        self.as_ctlv_ref().encode_vec()
    }

    /// Encodes this `CtlvRefMut` as an owned `String`.
    pub fn encode_string(&self) -> String {
        self.as_ctlv_ref().encode_string()
    }

    // XXX Rust makes it really hard to write this one
    // /// Decode a `CtlvRefMut` from the input buffer, returning it and the remaining input.
    // pub fn decode(input: &'a mut [u8])
    //               -> Result<(CtlvRefMut<'a>, &mut [u8]), (DecodeError, &mut [u8])> {
    //     let type_: u64;
    //     let length: usize;
    //     let remaining: &'a mut [u8];
    //
    //     match varu64::decode(input) {
    //         Err((_, tail)) if tail.len() == 0 => return Err((UnexpectedEndOfInput, input)),
    //         Err((e, tail)) => return Err((Type(e), tail)),
    //         Ok((t @ 0...127, tail)) => {
    //             type_ = t;
    //             length = 1 << (type_ >> 3);
    //             remaining = tail;
    //         }
    //         Ok((t, tail)) => {
    //             type_ = t;
    //
    //             match varu64::decode(tail) {
    //                 Err((e, tail2)) => return Err((Length(e), tail2)),
    //                 Ok((len, tail2)) => {
    //                     length = len as usize;
    //                     remaining = tail2;
    //                 }
    //             }
    //         }
    //     }
    //
    //     if remaining.len() < length {
    //         return Err((UnexpectedEndOfInput, remaining));
    //     } else {
    //         return Ok((CtlvRefMut {
    //                        type_,
    //                        value: &remaining[..length],
    //                    },
    //                    &remaining[length..]));
    //     }
    // }

    /// Returns a `CtlvRef` that borrows the same value as this `CtlvRefMut`.
    pub fn as_ctlv_ref(&self) -> CtlvRef {
        CtlvRef {
            type_: self.type_,
            value: &self.value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Assert that the given Ctlv encodes to the expected encoding, and that the
    // expected encoding decodes to the Ctlv.
    fn test_fixture(ctlv: &Ctlv, exp: &[u8]) {
        assert_eq!(ctlv.encoding_length(), exp.len());
        let mut foo = Vec::with_capacity(exp.len());
        foo.resize(exp.len(), 0);

        assert_eq!(ctlv.encode(&mut foo), exp.len());
        assert_eq!(foo, exp);

        let (dec, tail) = Ctlv::decode(exp).unwrap();
        assert_eq!(&dec, ctlv);
        assert_eq!(tail, &[][..]);
    }

    #[test]
    fn fixtures() {
        test_fixture(&Ctlv {
                          type_: 0,
                          value: vec![42],
                      },
                     &[0, 42]);

        test_fixture(&Ctlv {
                          type_: 1,
                          value: vec![42],
                      },
                     &[1, 42]);

        test_fixture(&Ctlv {
                          type_: 128,
                          value: vec![42],
                      },
                     &[128, 1, 42]);

        test_fixture(&Ctlv {
                          type_: 247,
                          value: vec![42],
                      },
                     &[247, 1, 42]);

        test_fixture(&Ctlv {
                          type_: 250,
                          value: vec![42],
                      },
                     &[248, 250, 1, 42]);

        assert_eq!(Ctlv::decode(&[]).unwrap_err(),
                   (UnexpectedEndOfInput, &[][..]));
        assert_eq!(Ctlv::decode(&[247, 248, 1, 42]).unwrap_err(),
                   (Length(VarU64Error::NonCanonical(1)), &[42][..]));
        assert_eq!(Ctlv::decode(&[248, 0, 1, 42]).unwrap_err(),
                   (Type(VarU64Error::NonCanonical(0)), &[1, 42][..]));
    }
}
