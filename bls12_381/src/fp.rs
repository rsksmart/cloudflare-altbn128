use crate::representation::{ElementRepr, RepresentationDecodingError};
use crate::traits::FieldElement;
use crate::traits::BitIterator;
use crate::traits::FieldExtension;
use crate::field::SizedPrimeField;
use crate::traits::ZeroAndOne;

pub struct Fp<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > {
    pub(crate) field: &'a F,
    pub(crate) repr: E
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > Clone for Fp<'a, E, F> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            field: &self.field,
            repr: self.repr
        }
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > Ord for Fp<'a, E, F> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // use non-montgommery form
        let modulus = self.field.modulus();
        let mont_inv = self.field.mont_inv();
        let this = self.repr.into_normal_repr(&modulus, mont_inv);
        let that = other.repr.into_normal_repr(&modulus, mont_inv);
        for (a, b) in this.as_ref().iter().rev().zip(that.as_ref().iter().rev()) {
            if a < b {
                return std::cmp::Ordering::Less
            } else if a > b {
                return std::cmp::Ordering::Greater
            }
        }

        std::cmp::Ordering::Equal
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > PartialEq for Fp<'a, E, F> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        for (a, b) in self.repr.as_ref().iter().rev().zip(other.repr.as_ref().iter().rev()) {
            if a != b {
                return false;
            }
        }

        true
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > Eq for Fp<'a, E, F> {
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > PartialOrd for Fp<'a, E, F> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > std::fmt::Debug for Fp<'a, E, F>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "0x")?;
        // for i in self.repr.as_ref().iter().rev() {
        for i in self.into_repr().as_ref().iter().rev() {
            write!(f, "{:016x}", *i)?;
        }

        Ok(())
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > std::fmt::Display for Fp<'a, E, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "0x")?;
        // for i in self.repr.as_ref().iter().rev() {
        for i in self.into_repr().as_ref().iter().rev() {
            write!(f, "{:016x}", *i)?;
        }

        Ok(())
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > Fp<'a, E, F> {
    // #[inline(always)]
    // pub fn zero(field: &'a F) -> Self {
    //     Self {
    //         field: field,
    //         repr: E::default()
    //     }
    // }

    // #[inline(always)]
    // pub fn one(field: &'a F) -> Self {
    //     Self {
    //         field: field,
    //         repr: field.mont_r()
    //     }
    // }

    pub fn from_repr(field: &'a F, repr: E) -> Result<Self, RepresentationDecodingError> {
        if field.is_valid_repr(&repr) {
            let mut r = Self {
                field: field,
                repr: repr
            };

            let r2 = Self {
                field: field,
                repr: *field.mont_r2()
            };

            r.mul_assign(&r2);

            Ok(r)
        } else {
            Err(RepresentationDecodingError::NotInField(format!("{}", repr)))
        }
    }

    pub fn from_raw_repr(field: &'a F, repr: E) -> Result<Self, RepresentationDecodingError> {
        if field.is_valid_repr(&repr) {
            let r = Self {
                field: field,
                repr: repr
            };

            Ok(r)
        } else {
            Err(RepresentationDecodingError::NotInField(format!("{}", repr)))
        }
    }

    pub fn into_repr(&self) -> E {
        let modulus = self.field.modulus();
        let mont_inv = self.field.mont_inv();
        self.repr.into_normal_repr(&modulus, mont_inv)
    }

    pub fn from_be_bytes(field: &'a F, bytes: &[u8], allow_padding: bool) -> Result<Self, RepresentationDecodingError> {
        let mut repr = E::default();
        if bytes.len() >= repr.as_ref().len() * 8 {
            repr.read_be(bytes).map_err(|e| RepresentationDecodingError::NotInField(format!("Failed to read big endian bytes, {}", e)))?;
        } else {
            if allow_padding {
                let mut padded = vec![0u8; repr.as_ref().len() * 8 - bytes.len()];
                padded.extend_from_slice(bytes);
                repr.read_be(&padded[..]).map_err(|e| RepresentationDecodingError::NotInField(format!("Failed to read big endian bytes, {}", e)))?;
            } else {
                repr.read_be(&bytes[..]).map_err(|e| RepresentationDecodingError::NotInField(format!("Failed to read big endian bytes without padding, {}", e)))?;
            }
        }
        Self::from_repr(field, repr)
    }

    pub fn from_be_bytes_with_padding(
        field: &'a F, 
        bytes: &[u8], 
        pad_beginning: bool, 
        expect_prepadded_beginning: bool
    ) -> Result<Self, RepresentationDecodingError> {
        let mut repr = E::default();
        let necessary_length = repr.as_ref().len() * 8;
        if bytes.len() >= necessary_length {
            if expect_prepadded_beginning {
                let start = bytes.len() - necessary_length;
                for &b in bytes[..start].iter() {
                    if b != 0u8 {
                        return Err(RepresentationDecodingError::NotInField("top bytes of the padded BE encoding are NOT zeroes".to_owned()));
                    }
                }
                repr.read_be(&bytes[start..]).map_err(|e| RepresentationDecodingError::NotInField(format!("Failed to read big endian bytes, {}", e)))?;
            } else {
                if bytes.len() != necessary_length {
                    return Err(RepresentationDecodingError::NotInField("supplied encoding is longer than expected".to_owned()));
                }
                repr.read_be(&bytes[..]).map_err(|e| RepresentationDecodingError::NotInField(format!("Failed to read big endian bytes, {}", e)))?;
            }
        } else {
            if pad_beginning {
                let mut padded = vec![0u8; necessary_length - bytes.len()];
                padded.extend_from_slice(bytes);
                repr.read_be(&padded[..]).map_err(|e| RepresentationDecodingError::NotInField(format!("Failed to read big endian bytes, {}", e)))?;
            } else {
                repr.read_be(&bytes[..]).map_err(|e| RepresentationDecodingError::NotInField(format!("Failed to read big endian bytes without padding, {}", e)))?;
            }
        }
        Self::from_repr(field, repr)
    }

    pub(crate) fn eea_inverse(&self) -> Option<Self> {
        if self.is_zero() {
            None
        } else {
            // Guajardo Kumar Paar Pelzl
            // Efficient Software-Implementation of Finite Fields with Applications to Cryptography
            // Algorithm 16 (BEA for Inversion in Fp)

            // also modified to run in a limited time

            let one = F::Repr::from(1);

            let modulus = *self.field.modulus();
            let mut u = self.repr;
            let mut v = modulus;
            let mut b = Self {
                field: &self.field,
                repr: *self.field.mont_r2()
            }; // Avoids unnecessary reduction step.
            let mut c = Self::zero(&self.field);

            let max_iterations = 2*self.field.mont_power();
            let mut found = false;

            let mut iterations = 0;

            while iterations < max_iterations {
                if u == one || v == one {
                    found = true;
                    break;
                }

                while u.is_even() && iterations < max_iterations {
                    iterations += 1;
                    u.div2();

                    if b.repr.is_even() {
                        b.repr.div2();
                    } else {
                        b.repr.add_nocarry(&modulus);
                        b.repr.div2();
                    }
                }

                while v.is_even() && iterations < max_iterations {
                    iterations += 1;
                    v.div2();

                    if c.repr.is_even() {
                        c.repr.div2();
                    } else {
                        c.repr.add_nocarry(&modulus);
                        c.repr.div2();
                    }
                }

                // u and v are not odd, so after subtraction one of them is even
                // and we'll get into one of the loops above and iterations counter
                // will increase
                if v < u {
                    u.sub_noborrow(&v);
                    b.sub_assign(&c);
                } else {
                    v.sub_noborrow(&u);
                    c.sub_assign(&b);
                }
            }

            // for _ in 0..max_iterations {
            //     if u == one || v == one {
            //         found = true;
            //         break;
            //     }

            //     for _ in 0..max_iterations {
            //         if !u.is_even() {
            //             break;
            //         }
            //         u.div2();

            //         if b.repr.is_even() {
            //             b.repr.div2();
            //         } else {
            //             b.repr.add_nocarry(&modulus);
            //             b.repr.div2();
            //         }
            //     }

            //     for _ in 0..max_iterations {
            //         if !v.is_even() {
            //             break;
            //         }
            //         v.div2();

            //         if c.repr.is_even() {
            //             c.repr.div2();
            //         } else {
            //             c.repr.add_nocarry(&modulus);
            //             c.repr.div2();
            //         }
            //     }

            //     if v < u {
            //         u.sub_noborrow(&v);
            //         b.sub_assign(&c);
            //     } else {
            //         v.sub_noborrow(&u);
            //         c.sub_assign(&b);
            //     }
            // }

            if !found {
                return None;
            }

            if u == one {
                Some(b)
            } else {
                Some(c)
            }
        }
    }

    /// Subtracts the modulus from this element if this element is not in the
    /// field. Only used interally.
    #[inline(always)]
    fn reduce(&mut self) {
        self.repr.reduce(&self.field.modulus());
        // if !self.field.is_valid_repr(self.repr) {
        //     self.repr.sub_noborrow(&self.field.modulus());
        // }
    }

    #[inline]
    fn mul_assign_with_partial_reduction(&mut self, other: &Self)
    {
        self.repr.mont_mul_assign_with_partial_reduction(&other.repr, &self.field.modulus(), self.field.mont_inv());
    }

    #[inline]
    fn square_with_partial_reduction(&mut self)
    {
        self.repr.mont_square_with_partial_reduction(&self.field.modulus(), self.field.mont_inv());
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > ZeroAndOne for Fp<'a, E, F> {
    type Params = &'a F;

    #[inline(always)]
    fn zero(field: &'a F) -> Self {
        Self {
            field: field,
            repr: E::default()
        }
    }

    #[inline(always)]
    fn one(field: &'a F) -> Self {
        Self {
            field: field,
            repr: *field.mont_r()
        }
    }
}

impl<'a, E: ElementRepr, F: SizedPrimeField<Repr = E> > FieldElement for Fp<'a, E, F> {
    /// Returns true iff this element is zero.
    #[inline]
    fn is_zero(&self) -> bool {
        self.repr.is_zero()
    }

    #[inline]
    fn add_assign(&mut self, other: &Self) {
        // This cannot exceed the backing capacity.
        self.repr.add_nocarry(&other.repr);

        // However, it may need to be reduced.
        self.reduce();
    }

    #[inline]
    fn double(&mut self) {
        // This cannot exceed the backing capacity.
        self.repr.mul2();

        // However, it may need to be reduced.
        self.reduce();
    }

    #[inline]
    fn sub_assign(&mut self, other: &Self) {
        // If `other` is larger than `self`, we'll need to add the modulus to self first.
        if other.repr > self.repr {
            self.repr.add_nocarry(&self.field.modulus());
        }

        self.repr.sub_noborrow(&other.repr);
    }

    #[inline]
    fn negate(&mut self) {
        if !self.is_zero() {
            let mut tmp = *self.field.modulus();
            tmp.sub_noborrow(&self.repr);
            self.repr = tmp;
        }
    }

    fn inverse(&self) -> Option<Self> {
        self.new_mont_inverse()
        // self.mont_inverse()
        // self.eea_inverse()
    }

    #[inline]
    fn mul_assign(&mut self, other: &Self)
    {
        self.repr.mont_mul_assign(&other.repr, &self.field.modulus(), self.field.mont_inv());
    }

    #[inline]
    fn square(&mut self)
    {
        self.repr.mont_square(&self.field.modulus(), self.field.mont_inv());
    }

    fn pow<S: AsRef<[u64]>>(&self, exp: S) -> Self {
        let mut res = Self::one(&self.field);

        let mut found_one = false;

        for i in BitIterator::new(exp) {
            if found_one {
                res.square();
            } else {
                found_one = i;
            }

            if i {
                res.mul_assign(self);
            }
        }

        res
    }

    // fn pow<S: AsRef<[u64]>>(&self, exp: S) -> Self {
    //     // This is powering with partial reduction!
    //     let mut res = Self::one(&self.field);

    //     let mut found_one = false;

    //     for i in BitIterator::new(exp) {
    //         if found_one {
    //             res.square_with_partial_reduction();
    //         } else {
    //             found_one = i;
    //         }

    //         if i {
    //             res.mul_assign_with_partial_reduction(self);
    //         }
    //     }

    //     res.reduce();

    //     res
    // }

    fn mul_by_nonresidue<EXT: FieldExtension<Element = Self>>(&mut self, for_extesion: &EXT) {
        for_extesion.multiply_by_non_residue(self);
    }

    fn conjugate(&mut self) {
        unreachable!();
    }

    #[inline]
    fn frobenius_map(&mut self, _power: usize) {
        // unreachable!();
    }
}