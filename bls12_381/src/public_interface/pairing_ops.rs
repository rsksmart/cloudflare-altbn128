// expected structure:

/// Every call has common parameters (may be redundant):
/// - Curve type
/// - Lengths of modulus (in bytes)
/// - Field modulus
/// - Curve A
/// - Curve B
/// - Lengths of group size (in bytes)
/// - Group size
/// - Type specific params
///
/// Assumptions:
/// - one byte for length encoding
/// 
/// 

use crate::weierstrass::curve::WeierstrassCurve;
use crate::weierstrass::{Group, CurveOverFpParameters, CurveOverFp2Parameters, CurveOverFp3Parameters};
use crate::pairings::*;
use crate::pairings::bls12::{Bls12Instance, Bls12InstanceParams};
use crate::pairings::bn::{BnInstance, BnInstanceParams};
use crate::pairings::mnt4::{MNT4Instance, MNT4InstanceParams};
use crate::pairings::mnt6::{MNT6Instance, MNT6InstanceParams};
use crate::representation::{ElementRepr};
use crate::traits::{FieldElement, ZeroAndOne};
use crate::extension_towers::*;
use crate::fp::Fp;
use crate::integers::*;

use super::decode_g1::*;
use super::decode_utils::*;
use super::decode_fp::*;
use super::decode_g2::*;
use super::constants::*;
use super::sane_limits::*;

use crate::errors::ApiError;

fn pairing_result_false() -> Vec<u8> {
    vec![0u8]
}

fn pairing_result_true() -> Vec<u8> {
    vec![1u8]
}

pub struct PublicPairingApi;

impl PairingApi for PublicPairingApi {
    fn pair(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        use crate::field::*;
        let (_curve_type, rest) = split(bytes, CURVE_TYPE_LENGTH, "Input should be longer than curve type encoding")?;
        let (_, modulus, _) = parse_modulus_and_length(&rest)?;
        let modulus_limbs = num_limbs_for_modulus(&modulus)?;

        let result: Result<Vec<u8>, ApiError> = expand_for_modulus_limbs!(modulus_limbs, PairingApiImplementation, bytes, pair); 

        result
    }
}

pub trait PairingApi {
    fn pair(bytes: &[u8]) -> Result<Vec<u8>, ApiError>;
}

pub(crate) struct PairingApiImplementation<FE: ElementRepr> {
    _marker_fe: std::marker::PhantomData<FE>,
}

impl<FE: ElementRepr> PairingApi for PairingApiImplementation<FE> {
    fn pair(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (curve_type, rest) = split(bytes, CURVE_TYPE_LENGTH, "Input should be longer than curve type encoding")?;

        match curve_type[0] {
            BLS12 => {
                PairingApiImplementation::<FE>::pair_bls12(&rest)
            },
            BN => {
                PairingApiImplementation::<FE>::pair_bn(&rest)
            },
            MNT4 => {
                PairingApiImplementation::<FE>::pair_mnt4(&rest)
            },
            MNT6 => {
                PairingApiImplementation::<FE>::pair_mnt6(&rest)
            },
            _ => {
                return Err(ApiError::InputError("Unknown curve type".to_owned()));
            }
        }
    }
}

impl<FE: ElementRepr>PairingApiImplementation<FE> {
    pub(crate) fn pair_bls12(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        use crate::extension_towers::fp2::{Fp2, Extension2};
        use crate::extension_towers::fp6_as_3_over_2::{Fp6, Extension3Over2};
        use crate::extension_towers::fp12_as_2_over3_over_2::{Fp12, Extension2Over3Over2};

        let (base_field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (a_fp, b_fp, rest) = parse_ab_in_base_field_from_encoding(&rest, modulus_len, &base_field)?;
        if !a_fp.is_zero() {
            return Err(ApiError::UnknownParameter("A parameter must be zero for BLS12 curve".to_owned()));
        }
        let (_order_len, order, rest) = parse_group_order_from_encoding(rest)?;
        let fp_params = CurveOverFpParameters::new(&base_field);
        let g1_curve = WeierstrassCurve::new(&order.as_ref(), a_fp, b_fp.clone(), &fp_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;


        // Now we need to expect:
        // - non-residue for Fp2
        // - non-residue for Fp6
        // - twist type M/D
        // - parameter X
        // - sign of X
        // - number of pairs
        // - list of encoded pairs

        let (fp_non_residue, rest) = decode_fp(&rest, modulus_len, &base_field)?;

        {
            if fp_non_residue.is_zero() {
                return Err(ApiError::InputError(format!("Non-residue for Fp2 is zero file {}, line {}", file!(), line!())));
            }
            let is_not_a_square = is_non_nth_root(&fp_non_residue, &modulus, 2u64);
            if !is_not_a_square {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Non-residue for Fp2 is actually a residue file {}, line {}", file!(), line!())));
                }
            }
        }

        // build an extension field
        let mut extension_2 = Extension2::new(fp_non_residue);
        extension_2.calculate_frobenius_coeffs(&modulus).map_err(|_| {
            ApiError::InputError("Failed to calculate Frobenius coeffs for Fp2".to_owned())
        })?;

        let (fp2_non_residue, rest) = decode_fp2(&rest, modulus_len, &extension_2)?;

        {
            if fp2_non_residue.is_zero() {
                return Err(ApiError::InputError(format!("Non-residue for Fp6(12) is zero, file {}, line {}", file!(), line!())));
            }
            let is_not_a_6th_root = is_non_nth_root_fp2(&fp2_non_residue, &modulus, 6u64);
            if !is_not_a_6th_root {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Non-residue for Fp6(12) is actually a residue, file {}, line {}", file!(), line!())));
                }
            }
        }

        let (twist_type, rest) = decode_twist_type(rest)?;

        let base_precomp = Fp6Fp12FrobeniusBaseElements::construct(
            &modulus, 
            &fp2_non_residue
        ).map_err(|_| {
            ApiError::UnknownParameter("Can not make base precomputations for Fp6/Fp12 frobenius".to_owned())
        })?;

        let mut extension_6 = Extension3Over2::new(fp2_non_residue.clone());
        {
            extension_6.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
                ApiError::UnknownParameter("Can not calculate Frobenius coefficients for Fp6".to_owned())
            })?;
        }

        let mut extension_12 = Extension2Over3Over2::new(Fp6::zero(&extension_6));
        {
            extension_12.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
                ApiError::InputError("Can not calculate Frobenius coefficients for Fp12".to_owned())
            })?;
        }

        let fp2_non_residue_inv = fp2_non_residue.inverse().ok_or(ApiError::UnexpectedZero("Fp2 non-residue must be invertible".to_owned()))?;
        let b_fp2 = match twist_type {
            TwistType::D => {
                let mut b_fp2 = fp2_non_residue_inv.clone();
                b_fp2.mul_by_fp(&b_fp);

                b_fp2
            },
            TwistType::M => {
                let mut b_fp2 = fp2_non_residue.clone();
                b_fp2.mul_by_fp(&b_fp);

                b_fp2
            },
        };

        let a_fp2 = Fp2::zero(&extension_2);

        let fp2_params = CurveOverFp2Parameters::new(&extension_2);
        let g2_curve = WeierstrassCurve::new(&order.as_ref(), a_fp2, b_fp2, &fp2_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (x, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_BLS12_X_BIT_LENGTH)?;
        if x.is_zero() {
            return Err(ApiError::InputError("Loop count parameters can not be zero".to_owned()));
        }

        if calculate_hamming_weight(&x.as_ref()) > MAX_BLS12_X_HAMMING {
            return Err(ApiError::InputError("X has too large hamming weight".to_owned()));
        }

        let (x_is_negative, rest) = decode_sign_is_negative(rest)?;

        let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
        let num_pairs = num_pairs_encoding[0] as usize;

        if num_pairs == 0 {
            if !crate::features::in_gas_metering() {
                return Err(ApiError::InputError("Zero pairs encoded".to_owned()));
            }
        }

        let mut global_rest = rest;

        let mut g1_points = vec![];
        let mut g2_points = vec![];

        for _ in 0..num_pairs {
            let (check_g1_subgroup, rest) = decode_boolean(&global_rest)?;
            let (g1, rest) = decode_g1_point_from_xy(&rest, modulus_len, &g1_curve)?;
            let (check_g2_subgroup, rest) = decode_boolean(&rest)?;
            let (g2, rest) = decode_g2_point_from_xy_in_fp2(&rest, modulus_len, &g2_curve)?;
            global_rest = rest;

            if !g1.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G1 point is not on curve".to_owned()));
                }
            }

            if !g2.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G2 point is not on curve".to_owned()));
                }
            }

            if check_g1_subgroup {
                if !g1.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if check_g2_subgroup {
                if !g2.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if !g1.is_zero() && !g2.is_zero() {
                g1_points.push(g1);
                g2_points.push(g2);
            }
        }

        if global_rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        debug_assert!(g1_points.len() == g2_points.len());
        if g1_points.len() == 0 {
            return Ok(pairing_result_true());
        }

        let engine_params = Bls12InstanceParams {
            x: &x.as_ref(),
            x_is_negative: x_is_negative,
            twist_type: twist_type,
            base_field: &base_field,
            curve: &g1_curve,
            curve_twist: &g2_curve,
            fp2_extension: &extension_2,
            fp6_extension: &extension_6,
            fp12_extension: &extension_12,
            force_no_naf: true
        };

        let engine = Bls12Instance::from_params(engine_params);

        let pairing_result = engine.pair(&g1_points, &g2_points);

        if pairing_result.is_none() {
            return Err(ApiError::UnknownParameter("Pairing engine returned no value".to_owned()));
        }

        let one_fp12 = Fp12::one(&extension_12);
        let pairing_result = pairing_result.unwrap();
        let result = if pairing_result == one_fp12 {
            pairing_result_true()
        } else {
            pairing_result_false()
        };

        Ok(result)
    }

    pub(crate) fn pair_bn(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        use crate::extension_towers::fp2::{Fp2, Extension2};
        use crate::extension_towers::fp6_as_3_over_2::{Fp6, Extension3Over2};
        use crate::extension_towers::fp12_as_2_over3_over_2::{Fp12, Extension2Over3Over2};

        let (base_field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (a_fp, b_fp, rest) = parse_ab_in_base_field_from_encoding(&rest, modulus_len, &base_field)?;
        if !a_fp.is_zero() {
            return Err(ApiError::UnknownParameter("A parameter must be zero for BN curve".to_owned()));
        }
        let (_order_len, order, rest) = parse_group_order_from_encoding(rest)?;
        let fp_params = CurveOverFpParameters::new(&base_field);
        let g1_curve = WeierstrassCurve::new(&order.as_ref(), a_fp, b_fp.clone(), &fp_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;


        // Now we need to expect:
        // - non-residue for Fp2
        // - non-residue for Fp6
        // - twist type M/D
        // - parameter U
        // - sign of U
        // - number of pairs
        // - list of encoded pairs
        // U is used instead of x for convention of go-ethereum people :)

        let (fp_non_residue, rest) = decode_fp(&rest, modulus_len, &base_field)?;

        {
            if fp_non_residue.is_zero() {
                return Err(ApiError::InputError(format!("Non-residue for Fp2 is zero file {}, line {}", file!(), line!())));
            }
            let is_not_a_square = is_non_nth_root(&fp_non_residue, &modulus, 2u64);
            if !is_not_a_square {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Non-residue for Fp2 is actually a residue file {}, line {}", file!(), line!())));
                }
            }
        }

        // build an extension field
        let mut extension_2 = Extension2::new(fp_non_residue);
        extension_2.calculate_frobenius_coeffs(&modulus).map_err(|_| {
            ApiError::InputError("Failed to calculate Frobenius coeffs for Fp2".to_owned())
        })?;

        let (fp2_non_residue, rest) = decode_fp2(&rest, modulus_len, &extension_2)?;

        {
            if fp2_non_residue.is_zero() {
                return Err(ApiError::InputError(format!("Non-residue for Fp6(12) is zero, file {}, line {}", file!(), line!())));
            }
            let is_not_a_6th_root = is_non_nth_root_fp2(&fp2_non_residue, &modulus, 6u64);
            if !is_not_a_6th_root {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Non-residue for Fp6(12) is actually a residue, file {}, line {}", file!(), line!())));
                }
            }
        }

        let (twist_type, rest) = decode_twist_type(&rest)?;

        let base_precomp = Fp6Fp12FrobeniusBaseElements::construct(
            &modulus, 
            &fp2_non_residue
        ).map_err(|_| {
            ApiError::UnknownParameter("Can not make base precomputations for Fp6/Fp12 frobenius".to_owned())
        })?;

        let mut extension_6 = Extension3Over2::new(fp2_non_residue.clone());
        {
            extension_6.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
                ApiError::UnknownParameter("Can not calculate Frobenius coefficients for Fp6".to_owned())
            })?;
        }

        let mut extension_12 = Extension2Over3Over2::new(Fp6::zero(&extension_6));
        {
            extension_12.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
                ApiError::InputError("Can not calculate Frobenius coefficients for Fp12".to_owned())
            })?;
        }

        let fp2_non_residue_inv = fp2_non_residue.inverse().ok_or(ApiError::UnexpectedZero("Fp2 non-residue must be invertible".to_owned()))?;

        let b_fp2 = match twist_type {
            TwistType::D => {
                let mut b_fp2 = fp2_non_residue_inv.clone();
                b_fp2.mul_by_fp(&b_fp);

                b_fp2
            },
            TwistType::M => {
                let mut b_fp2 = fp2_non_residue.clone();
                b_fp2.mul_by_fp(&b_fp);

                b_fp2
            },
        };

        let a_fp2 = Fp2::zero(&extension_2);

        let fp2_params = CurveOverFp2Parameters::new(&extension_2);
        let g2_curve = WeierstrassCurve::new(&order.as_ref(), a_fp2, b_fp2, &fp2_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (u, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_BN_U_BIT_LENGTH)?;
        if u.is_zero() {
            return Err(ApiError::InputError("Loop count parameters can not be zero".to_owned()));
        }

        let (u_is_negative, rest) = decode_sign_is_negative(rest)?;

        let two = MaxLoopParametersUint::from(2u64);
        let six = MaxLoopParametersUint::from(6u64);

        // we need only absolute value of 6u+2, so manually handle negative and positive U
        let six_u_plus_two = if u_is_negative {
            let six_u_plus_two = (six * u) - two;

            six_u_plus_two
        } else {
            let six_u_plus_two = (six * u) + two;

            six_u_plus_two
        };

        if calculate_hamming_weight(&six_u_plus_two.as_ref()) > MAX_BN_SIX_U_PLUS_TWO_HAMMING {
            return Err(ApiError::InputError("|6*U + 2| has too large hamming weight".to_owned()));
        }

        let p_minus_one_over_2 = (modulus - MaxFieldUint::from(1u64)) >> 1;

        let fp2_non_residue_in_p_minus_one_over_2 = fp2_non_residue.pow(p_minus_one_over_2.as_ref());

        let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
        let num_pairs = num_pairs_encoding[0] as usize;

        if num_pairs == 0 {
            if !crate::features::in_gas_metering() {
                return Err(ApiError::InputError("Zero pairs encoded".to_owned()));
            }
        }

        let mut global_rest = rest;

        let mut g1_points = vec![];
        let mut g2_points = vec![];

        for _ in 0..num_pairs {
            let (check_g1_subgroup, rest) = decode_boolean(&global_rest)?;
            let (g1, rest) = decode_g1_point_from_xy(&rest, modulus_len, &g1_curve)?;
            let (check_g2_subgroup, rest) = decode_boolean(&rest)?;
            let (g2, rest) = decode_g2_point_from_xy_in_fp2(&rest, modulus_len, &g2_curve)?;
            global_rest = rest;

            if !g1.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G1 point is not on curve".to_owned()));
                }
            }

            if !g2.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G2 point is not on curve".to_owned()));
                }
            }

            if check_g1_subgroup {
                if !g1.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if check_g2_subgroup {
                if !g2.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if !g1.is_zero() && !g2.is_zero() {
                g1_points.push(g1);
                g2_points.push(g2);
            }
        }

        if global_rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        debug_assert!(g1_points.len() == g2_points.len());
        if g1_points.len() == 0 {
            return Ok(pairing_result_true());
        }

        let engine_params = BnInstanceParams {
            u: &u.as_ref(),
            six_u_plus_2: &six_u_plus_two.as_ref(),
            u_is_negative: u_is_negative,
            twist_type: twist_type,
            base_field: &base_field,
            curve: &g1_curve,
            curve_twist: &g2_curve,
            fp2_extension: &extension_2,
            fp6_extension: &extension_6,
            fp12_extension: &extension_12,
            non_residue_in_p_minus_one_over_2: fp2_non_residue_in_p_minus_one_over_2,
            force_no_naf: true
        };

        let engine = BnInstance::from_params(engine_params);

        let pairing_result = engine.pair(&g1_points, &g2_points);

        if pairing_result.is_none() {
            return Err(ApiError::UnknownParameter("Pairing engine returned no value".to_owned()));
        }

        let one_fp12 = Fp12::one(&extension_12);
        let pairing_result = pairing_result.unwrap();
        let result = if pairing_result == one_fp12 {
            pairing_result_true()
        } else {
            pairing_result_false()
        };

        Ok(result)
    }

    pub(crate) fn pair_mnt6(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        use crate::extension_towers::fp3::{Fp3, Extension3};
        use crate::extension_towers::fp6_as_2_over_3::{Fp6, Extension2Over3};

        let (base_field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (a_fp, b_fp, rest) = parse_ab_in_base_field_from_encoding(&rest, modulus_len, &base_field)?;
        let (_order_len, order, rest) = parse_group_order_from_encoding(rest)?;
        let fp_params = CurveOverFpParameters::new(&base_field);
        let g1_curve = WeierstrassCurve::new(&order.as_ref(), a_fp.clone(), b_fp.clone(), &fp_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        // Now we need to expect:
        // - non-residue for Fp3
        // now separate Miller loop params
        // - parameter X
        // - sign of X 
        // Final exp params
        // - exp_w0
        // - exp_w1
        // - exp_w0_is_negative
        // - number of pairs
        // - list of encoded pairs

        let (fp_non_residue, rest) = decode_fp(&rest, modulus_len, &base_field)?;

        {
            if fp_non_residue.is_zero() {
                return Err(ApiError::InputError(format!("Non-residue for Fp3 is zero file {}, line {}", file!(), line!())));
            }
            let is_not_a_root = is_non_nth_root(&fp_non_residue, &modulus, 6u64);
            if !is_not_a_root {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Non-residue for Fp3 is actually a residue, file {}, line {}", file!(), line!())));
                }
            }
        }

        let base_precomp = Fp3Fp6FrobeniusBaseElements::construct(
            &modulus, &fp_non_residue
        ).map_err(|_| {
            ApiError::UnknownParameter("Can not make base precomputations for Fp3/Fp6 frobenius".to_owned())
        })?;

        // build an extension field
        let mut extension_3 = Extension3::new(fp_non_residue);
        extension_3.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
            ApiError::InputError("Failed to calculate Frobenius coeffs for Fp3".to_owned())
        })?;

        let mut extension_6 = Extension2Over3::new(Fp3::zero(&extension_3));

        {
            extension_6.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
                ApiError::UnknownParameter("Can not calculate Frobenius coefficients for Fp6".to_owned())
            })?;
        }

        let one = Fp::one(&base_field);

        let mut twist = Fp3::zero(&extension_3);
        twist.c1 = one.clone();

        let mut twist_squared = twist.clone();
        twist_squared.square();

        let mut twist_cubed = twist_squared.clone();
        twist_cubed.mul_assign(&twist);

        let mut a_fp3 = twist_squared.clone();
        a_fp3.mul_by_fp(&a_fp);

        let mut b_fp3 = twist_cubed.clone();
        b_fp3.mul_by_fp(&b_fp);

        let fp3_params = CurveOverFp3Parameters::new(&extension_3);
        let g2_curve = WeierstrassCurve::new(&order.as_ref(), a_fp3, b_fp3, &fp3_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (x, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_ATE_LOOP_COUNT)?;
        if x.is_zero() {
            return Err(ApiError::InputError("Ate loop count parameters can not be zero".to_owned()));
        }

        if calculate_hamming_weight(&x.as_ref()) > MAX_ATE_PAIRING_ATE_LOOP_COUNT_HAMMING {
            return Err(ApiError::InputError("X has too large hamming weight".to_owned()));
        }

        let (x_is_negative, rest) = decode_sign_is_negative(rest)?;

        let (exp_w0, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_FINAL_EXP_W0_BIT_LENGTH)?;
        if exp_w0.is_zero() {
            return Err(ApiError::InputError("Final exp w0 loop count parameters can not be zero".to_owned()));
        }

        let (exp_w1, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_FINAL_EXP_W1_BIT_LENGTH)?;
        if exp_w1.is_zero() {
            return Err(ApiError::InputError("Final exp w1 loop count parameters can not be zero".to_owned()));
        }

        let (exp_w0_is_negative, rest) = decode_sign_is_negative(rest)?;

        let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
        let num_pairs = num_pairs_encoding[0] as usize;

        if num_pairs == 0 {
            if !crate::features::in_gas_metering() {
                return Err(ApiError::InputError("Zero pairs encoded".to_owned()));
            }
        }

        let mut global_rest = rest;

        let mut g1_points = vec![];
        let mut g2_points = vec![];

        for _ in 0..num_pairs {
            let (check_g1_subgroup, rest) = decode_boolean(&global_rest)?;
            let (g1, rest) = decode_g1_point_from_xy(&rest, modulus_len, &g1_curve)?;
            let (check_g2_subgroup, rest) = decode_boolean(&rest)?;
            let (g2, rest) = decode_g2_point_from_xy_in_fp3(&rest, modulus_len, &g2_curve)?;
            global_rest = rest;

            if !g1.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G1 point is not on curve".to_owned()));
                }
            }

            if !g2.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G2 point is not on curve".to_owned()));
                }
            }

            if check_g1_subgroup {
                if !g1.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if check_g2_subgroup {
                if !g2.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if !g1.is_zero() && !g2.is_zero() {
                g1_points.push(g1);
                g2_points.push(g2);
            }
        }

        if global_rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        debug_assert!(g1_points.len() == g2_points.len());
        if g1_points.len() == 0 {
            return Ok(pairing_result_true());
        }

        let engine_params = MNT6InstanceParams {
            x: &x.as_ref(),
            x_is_negative: x_is_negative,
            exp_w0: exp_w0.as_ref(),
            exp_w1: exp_w1.as_ref(),
            exp_w0_is_negative: exp_w0_is_negative,
            base_field: &base_field,
            curve: &g1_curve,
            curve_twist: &g2_curve,
            twist: twist,
            fp3_extension: &extension_3,
            fp6_extension: &extension_6,
            force_no_naf: true
        };

        let engine = MNT6Instance::from_params(engine_params);

        let pairing_result = engine.pair(&g1_points, &g2_points);

        if pairing_result.is_none() {
            return Err(ApiError::UnknownParameter("Pairing engine returned no value".to_owned()));
        }

        let one_fp6 = Fp6::one(&extension_6);
        let pairing_result = pairing_result.unwrap();
        let result = if pairing_result == one_fp6 {
            pairing_result_true()
        } else {
            pairing_result_false()
        };

        Ok(result)
    }

    pub(crate) fn pair_mnt4(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        use crate::extension_towers::fp2::{Fp2, Extension2};
        use crate::extension_towers::fp4_as_2_over_2::{Fp4, Extension2Over2};

        let (base_field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (a_fp, b_fp, rest) = parse_ab_in_base_field_from_encoding(&rest, modulus_len, &base_field)?;
        let (_order_len, order, rest) = parse_group_order_from_encoding(rest)?;
        let fp_params = CurveOverFpParameters::new(&base_field);
        let g1_curve = WeierstrassCurve::new(&order.as_ref(), a_fp.clone(), b_fp.clone(), &fp_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        // Now we need to expect:
        // - non-residue for Fp2
        // now separate Miller loop params
        // - parameter X
        // - sign of X 
        // Final exp params
        // - exp_w0
        // - exp_w1
        // - exp_w0_is_negative
        // - number of pairs
        // - list of encoded pairs

        let (fp_non_residue, rest) = decode_fp(&rest, modulus_len, &base_field)?;

        {
            if fp_non_residue.is_zero() {
                return Err(ApiError::InputError(format!("Non-residue for Fp2 is zero file {}, line {}", file!(), line!())));
            }
            let is_not_a_root = is_non_nth_root(&fp_non_residue, &modulus, 4u64);
            if !is_not_a_root {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Non-residue for Fp2 is actually a residue, file {}, line {}", file!(), line!())));
                }
            }
        }

        let base_precomp = Fp2Fp4FrobeniusBaseElements::construct(
            &modulus, &fp_non_residue
        ).map_err(|_| {
            ApiError::UnknownParameter("Can not make base precomputations for Fp3/Fp6 frobenius".to_owned())
        })?;

        // build an extension field
        let mut extension_2 = Extension2::new(fp_non_residue);
        extension_2.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
            ApiError::InputError("Failed to calculate Frobenius coeffs for Fp2".to_owned())
        })?;

        let mut extension_4 = Extension2Over2::new(Fp2::zero(&extension_2));

        {
            extension_4.calculate_frobenius_coeffs_with_precomp(&base_precomp).map_err(|_| {
                ApiError::UnknownParameter("Can not calculate Frobenius coefficients for Fp4".to_owned())
            })?;
        }

        // // build an extension field

        let one = Fp::one(&base_field);

        let mut twist = Fp2::zero(&extension_2);
        twist.c1 = one.clone();

        let mut twist_squared = twist.clone();
        twist_squared.square();

        let mut twist_cubed = twist_squared.clone();
        twist_cubed.mul_assign(&twist);

        let mut a_fp2 = twist_squared.clone();
        a_fp2.mul_by_fp(&a_fp);

        let mut b_fp2 = twist_cubed.clone();
        b_fp2.mul_by_fp(&b_fp);

        let fp2_params = CurveOverFp2Parameters::new(&extension_2);
        let g2_curve = WeierstrassCurve::new(&order.as_ref(), a_fp2, b_fp2, &fp2_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (x, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_ATE_LOOP_COUNT)?;
        if x.is_zero() {
            return Err(ApiError::InputError("Ate pairing loop count parameters can not be zero".to_owned()));
        }

        if calculate_hamming_weight(&x.as_ref()) > MAX_ATE_PAIRING_ATE_LOOP_COUNT_HAMMING {
            return Err(ApiError::InputError("X has too large hamming weight".to_owned()));
        }

        let (x_is_negative, rest) = decode_sign_is_negative(rest)?;

        let (exp_w0, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_FINAL_EXP_W0_BIT_LENGTH)?;
        if exp_w0.is_zero() {
            return Err(ApiError::InputError("Final exp w0 loop count parameters can not be zero".to_owned()));
        }
        let (exp_w1, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_FINAL_EXP_W1_BIT_LENGTH)?;
        if exp_w1.is_zero() {
            return Err(ApiError::InputError("Final exp w1 loop count parameters can not be zero".to_owned()));
        }

        let (exp_w0_is_negative, rest) = decode_sign_is_negative(rest)?;

        let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
        let num_pairs = num_pairs_encoding[0] as usize;

        if num_pairs == 0 {
            if !crate::features::in_gas_metering() {
                return Err(ApiError::InputError("Zero pairs encoded".to_owned()));
            }
        }

        let mut global_rest = rest;

        let mut g1_points = vec![];
        let mut g2_points = vec![];

        for _ in 0..num_pairs {
            let (check_g1_subgroup, rest) = decode_boolean(&global_rest)?;
            let (g1, rest) = decode_g1_point_from_xy(&rest, modulus_len, &g1_curve)?;
            let (check_g2_subgroup, rest) = decode_boolean(&rest)?;
            let (g2, rest) = decode_g2_point_from_xy_in_fp2(&rest, modulus_len, &g2_curve)?;
            global_rest = rest;

            if !g1.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G1 point is not on curve".to_owned()));
                }
            }

            if !g2.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError("G2 point is not on curve".to_owned()));
                }
            }

            if check_g1_subgroup {
                if !g1.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if check_g2_subgroup {
                if !g2.check_correct_subgroup() {
                    if !crate::features::in_fuzzing_or_gas_metering() {
                        return Err(ApiError::InputError("G1 or G2 point is not in the expected subgroup".to_owned()));
                    }
                }
            }

            if !g1.is_zero() && !g2.is_zero() {
                g1_points.push(g1);
                g2_points.push(g2);
            }
        }

        if global_rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        debug_assert!(g1_points.len() == g2_points.len());
        if g1_points.len() == 0 {
            return Ok(pairing_result_true());
        }

        let engine = MNT4InstanceParams {
            x: &x.as_ref(),
            x_is_negative: x_is_negative,
            exp_w0: &exp_w0.as_ref(),
            exp_w1: &exp_w1.as_ref(),
            exp_w0_is_negative: exp_w0_is_negative,
            base_field: &base_field,
            curve: &g1_curve,
            curve_twist: &g2_curve,
            twist: twist,
            fp2_extension: &extension_2,
            fp4_extension: &extension_4,
            force_no_naf: true
        };

        let engine = MNT4Instance::from_params(engine);

        let pairing_result = engine.pair(&g1_points, &g2_points);

        if pairing_result.is_none() {
            return Err(ApiError::UnknownParameter("Pairing engine returned no value".to_owned()));
        }

        let one_fp4 = Fp4::one(&extension_4);
        let pairing_result = pairing_result.unwrap();
        let result = if pairing_result == one_fp4 {
            pairing_result_true()
        } else {
            pairing_result_false()
        };

        Ok(result)
    }
}