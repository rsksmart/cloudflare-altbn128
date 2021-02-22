use byteorder;
use std::fmt;
use std::error::Error;
use std::io::{self, Read, Write};

/// This trait represents a wrapper around a biginteger which can encode any element of a particular
/// prime field. It is a smart wrapper around a sequence of `u64` limbs, least-significant digit
/// first.
pub trait ElementRepr:
    Sized
    + Copy
    + Clone
    + Eq
    + Ord
    + Send
    + Sync
    + Default
    + fmt::Debug
    + fmt::Display
    + 'static
    + AsRef<[u64]>
    + AsMut<[u64]>
    + From<u64>
{
    const NUM_LIMBS: usize;

    /// Subtract another represetation from this one.
    fn sub_noborrow(&mut self, other: &Self);

    /// Add another representation to this one.
    fn add_nocarry(&mut self, other: &Self);

    /// Compute the number of bits needed to encode this number. Always a
    /// multiple of 64.
    fn num_bits(&self) -> u32;

    /// Returns true iff this number is zero.
    fn is_zero(&self) -> bool;

    /// Returns true iff this number is odd.
    fn is_odd(&self) -> bool;

    /// Returns true iff this number is even.
    fn is_even(&self) -> bool;

    /// Performs a rightwise bitshift of this number, effectively dividing
    /// it by 2.
    fn div2(&mut self);

    /// Performs a rightwise bitshift of this number by some amount.
    fn shr(&mut self, amt: u32);

    /// Performs a leftwise bitshift of this number, effectively multiplying
    /// it by 2. Overflow is ignored.
    fn mul2(&mut self);

    /// Performs a leftwise bitshift of this number by some amount.
    fn shl(&mut self, amt: u32);

    /// Writes this `PrimeFieldRepr` as a big endian integer.
    fn write_be<W: Write>(&self, mut writer: W) -> io::Result<()> {
        use byteorder::{BigEndian, WriteBytesExt};

        for digit in self.as_ref().iter().rev() {
            writer.write_u64::<BigEndian>(*digit)?;
        }

        Ok(())
    }

    /// Reads a big endian integer into this representation.
    fn read_be<R: Read>(&mut self, mut reader: R) -> io::Result<()> {
        use byteorder::{BigEndian, ReadBytesExt};

        for digit in self.as_mut().iter_mut().rev() {
            *digit = reader.read_u64::<BigEndian>()?;
        }

        Ok(())
    }

    /// Writes this `PrimeFieldRepr` as a little endian integer.
    fn write_le<W: Write>(&self, mut writer: W) -> io::Result<()> {
        use byteorder::{LittleEndian, WriteBytesExt};

        for digit in self.as_ref().iter() {
            writer.write_u64::<LittleEndian>(*digit)?;
        }

        Ok(())
    }

    /// Reads a little endian integer into this representation.
    fn read_le<R: Read>(&mut self, mut reader: R) -> io::Result<()> {
        use byteorder::{LittleEndian, ReadBytesExt};

        for digit in self.as_mut().iter_mut() {
            *digit = reader.read_u64::<LittleEndian>()?;
        }

        Ok(())
    }

    // these two functions are mixing a representation and (Montgommery) form,
    // but it's a necessary evil
    fn mont_mul_assign(&mut self, other: &Self, modulus: &Self, mont_inv: u64);
    fn mont_square(&mut self, modulus: &Self, mont_inv: u64);
    fn mont_mul_assign_with_partial_reduction(&mut self, other: &Self, modulus: &Self, mont_inv: u64);
    fn mont_square_with_partial_reduction(&mut self, modulus: &Self, mont_inv: u64);
    fn into_normal_repr(&self, modulus: &Self, mont_inv: u64) -> Self;
    fn reduce(&mut self, modulus: &Self);
}

pub trait IntoWnaf {
    fn wnaf(&self, window: u32) -> Vec<i64>;
}

/// An error that may occur when trying to interpret a `PrimeFieldRepr` as a
/// `PrimeField` element.
#[derive(Debug)]
pub enum RepresentationDecodingError {
    /// The encoded value is not in the field
    NotInField(String),
}

impl Error for RepresentationDecodingError {
    fn description(&self) -> &str {
        match *self {
            RepresentationDecodingError::NotInField(..) => "not an element of the field",
        }
    }
}

impl fmt::Display for RepresentationDecodingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            RepresentationDecodingError::NotInField(ref repr) => {
                write!(f, "{} is not an element of the field", repr)
            }
        }
    }
}

pub(crate) fn num_bits(repr: &[u64]) -> u32 {
    let mut bits = (64 * repr.len()) as u32;
    for limb in repr.iter().rev() {
        let limb = *limb;
        if limb == 0 {
            bits -= 64;
            continue;
        } else {
            bits -= limb.leading_zeros();
            break;
        }
    }

    bits
}

pub(crate) fn right_shift_representation(repr: &mut [u64], shift: u64) {
    let num_libs = repr.len();
    for i in 0..(num_libs - 1) {
        repr[i] = (repr[i] >> shift) | (repr[i+1] << (64 - shift));
    }
    repr[num_libs - 1] = repr[num_libs - 1] >> shift;
}