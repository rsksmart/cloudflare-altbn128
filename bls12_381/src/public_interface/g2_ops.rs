use crate::weierstrass::{Group, CurveOverFp2Parameters, CurveOverFp3Parameters};
use crate::weierstrass::curve::{CurvePoint, WeierstrassCurve};
use crate::representation::ElementRepr;
use crate::multiexp::peppinger;

use crate::field::*;

use super::decode_utils::*;
use super::decode_g2::*;
use super::decode_g1::*;
use super::constants::*;
use super::decode_fp::*;

use crate::errors::ApiError;

/// Every call has common parameters (may be redundant):
/// - Lengths of modulus (in bytes)
/// - Field modulus
/// - Extension degree (2/3)
/// - Non-residue
/// - Curve A in Fpk
/// - Curve B in Fpk
/// - Length of a scalar field (curve order) (in bytes)
/// - Curve order

pub trait G2Api {
    fn add_points(bytes: &[u8]) -> Result<Vec<u8>, ApiError>;
    fn mul_point(bytes: &[u8]) -> Result<Vec<u8>, ApiError>;
    fn multiexp(bytes: &[u8]) -> Result<Vec<u8>, ApiError>;
}

pub struct G2ApiImplementationFp2<FE: ElementRepr> {
    _marker_fe: std::marker::PhantomData<FE>,
}

impl<FE: ElementRepr> G2Api for G2ApiImplementationFp2<FE> {
    fn add_points(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (extension_2, rest) = create_fp2_extension(rest, &modulus, modulus_len, &field, false)?;
        let (a, b, rest) = parse_ab_in_fp2_from_encoding(&rest, modulus_len, &extension_2)?;
        let (_order_len, order, rest) = parse_group_order_from_encoding(rest)?;

        let fp2_params = CurveOverFp2Parameters::new(&extension_2);

        let curve = WeierstrassCurve::new(&order.as_ref(), a, b, &fp2_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (mut p_0, rest) = decode_g2_point_from_xy_in_fp2(rest, modulus_len, &curve)?;
        let (p_1, rest) = decode_g2_point_from_xy_in_fp2(rest, modulus_len, &curve)?;

        if rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        if !p_0.is_on_curve() {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Point 0 is not on curve, file {}, line {}", file!(), line!())));
            }
        }
        if !p_1.is_on_curve() {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Point 1 is not on curve, file {}, line {}", file!(), line!())));
            }
        }

        p_0.add_assign(&p_1);

        serialize_g2_point_in_fp2(modulus_len, &p_0)   
    }

    fn mul_point(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (extension_2, rest) = create_fp2_extension(rest, &modulus, modulus_len, &field, false)?;
        let (a, b, rest) = parse_ab_in_fp2_from_encoding(&rest, modulus_len, &extension_2)?;
        let (order_len, order, rest) = parse_group_order_from_encoding(rest)?;

        let fp2_params = CurveOverFp2Parameters::new(&extension_2);

        let curve = WeierstrassCurve::new(&order.as_ref(), a, b, &fp2_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (p_0, rest) = decode_g2_point_from_xy_in_fp2(rest, modulus_len, &curve)?;
        let (scalar, rest) = decode_scalar_representation(rest, order_len)?;

        if rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        if !p_0.is_on_curve() {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Point is not on curve, file {}, line {}", file!(), line!())));
            }
        }

        let p = p_0.mul(&scalar);

        serialize_g2_point_in_fp2(modulus_len, &p)   
    }

    fn multiexp(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (extension_2, rest) = create_fp2_extension(&rest, &modulus, modulus_len, &field, false)?;
        let (a, b, rest) = parse_ab_in_fp2_from_encoding(&rest, modulus_len, &extension_2)?;
        let (order_len, order, rest) = parse_group_order_from_encoding(rest)?;

        let fp2_params = CurveOverFp2Parameters::new(&extension_2);

        let curve = WeierstrassCurve::new(&order.as_ref(), a, b, &fp2_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
        let num_pairs = num_pairs_encoding[0] as usize;

        if num_pairs == 0 {
            return Err(ApiError::InputError("Invalid number of pairs".to_owned()));
        }

        let expected_pair_len = 4*modulus_len + order_len;
        if rest.len() != expected_pair_len * num_pairs {
            return Err(ApiError::InputError("Input length is invalid for number of pairs".to_owned()));
        }

        let mut global_rest = rest;
        let mut bases = Vec::with_capacity(num_pairs);
        let mut scalars = Vec::with_capacity(num_pairs);

        for _ in 0..num_pairs {
            let (p, local_rest) = decode_g2_point_from_xy_in_fp2(global_rest, modulus_len, &curve)?;
            if !p.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Point is not on curve, file {}, line {}", file!(), line!())));
                }
            }
            let (scalar, local_rest) = decode_scalar_representation(local_rest, order_len)?;
            bases.push(p);
            scalars.push(scalar);
            global_rest = local_rest;
        }

        if global_rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        if bases.len() != scalars.len() || bases.len() == 0 {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Multiexp with empty input pairs, file {}, line {}", file!(), line!())));
            } else {
                let result = CurvePoint::zero(&curve);
                return serialize_g2_point_in_fp2(modulus_len, &result);
            }
        } 

        let result = peppinger(&bases, scalars);

        serialize_g2_point_in_fp2(modulus_len, &result)   
    }
}

pub struct G2ApiImplementationFp3<FE: ElementRepr> {
    _marker_fe: std::marker::PhantomData<FE>,
}

impl<FE: ElementRepr> G2Api for G2ApiImplementationFp3<FE> {
    fn add_points(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (extension_3, rest) = create_fp3_extension(rest, &modulus, modulus_len, &field, false)?;
        let (a, b, rest) = parse_ab_in_fp3_from_encoding(&rest, modulus_len, &extension_3)?;
        let (_order_len, order, rest) = parse_group_order_from_encoding(rest)?;

        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&order.as_ref(), a, b, &fp3_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (mut p_0, rest) = decode_g2_point_from_xy_in_fp3(rest, modulus_len, &curve)?;
        let (p_1, rest) = decode_g2_point_from_xy_in_fp3(rest, modulus_len, &curve)?;

        if rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        if !p_0.is_on_curve() {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Point 0 is not on curve, file {}, line {}", file!(), line!())));
            }
        }
        if !p_1.is_on_curve() {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Point 1 is not on curve, file {}, line {}", file!(), line!())));
            }
        }

        p_0.add_assign(&p_1);

        serialize_g2_point_in_fp3(modulus_len, &p_0)
    }

    fn mul_point(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (extension_3, rest) = create_fp3_extension(rest, &modulus, modulus_len, &field, false)?;
        let (a, b, rest) = parse_ab_in_fp3_from_encoding(&rest, modulus_len, &extension_3)?;
        let (order_len, order, rest) = parse_group_order_from_encoding(rest)?;

        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&order.as_ref(), a, b, &fp3_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (p_0, rest) = decode_g2_point_from_xy_in_fp3(rest, modulus_len, &curve)?;
        let (scalar, rest) = decode_scalar_representation(rest, order_len)?;

        if rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        if !p_0.is_on_curve() {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Point is not on curve, file {}, line {}", file!(), line!())));
            }
        }

        let p = p_0.mul(&scalar);

        serialize_g2_point_in_fp3(modulus_len, &p)   
    }

    fn multiexp(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (field, modulus_len, modulus, rest) = parse_base_field_from_encoding::<FE>(&bytes)?;
        let (extension_3, rest) = create_fp3_extension(&rest, &modulus, modulus_len, &field, false)?;
        let (a, b, rest) = parse_ab_in_fp3_from_encoding(&rest, modulus_len, &extension_3)?;
        let (order_len, order, rest) = parse_group_order_from_encoding(rest)?;

        let fp3_params = CurveOverFp3Parameters::new(&extension_3);

        let curve = WeierstrassCurve::new(&order.as_ref(), a, b, &fp3_params).map_err(|_| {
            ApiError::InputError("Curve shape is not supported".to_owned())
        })?;

        let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
        let num_pairs = num_pairs_encoding[0] as usize;

        if num_pairs == 0 {
            return Err(ApiError::InputError("Invalid number of pairs".to_owned()));
        }

        let expected_pair_len = 6*modulus_len + order_len;
        if rest.len() != expected_pair_len * num_pairs {
            return Err(ApiError::InputError("Input length is invalid for number of pairs".to_owned()));
        }

        let mut global_rest = rest;
        let mut bases = Vec::with_capacity(num_pairs);
        let mut scalars = Vec::with_capacity(num_pairs);

        for _ in 0..num_pairs {
            let (p, local_rest) = decode_g2_point_from_xy_in_fp3(global_rest, modulus_len, &curve)?;
            if !p.is_on_curve() {
                if !crate::features::in_fuzzing_or_gas_metering() {
                    return Err(ApiError::InputError(format!("Point is not on curve, file {}, line {}", file!(), line!())));
                }
            }
            let (scalar, local_rest) = decode_scalar_representation(local_rest, order_len)?;
            bases.push(p);
            scalars.push(scalar);
            global_rest = local_rest;
        }

        if global_rest.len() != 0 {
            return Err(ApiError::InputError("Input contains garbage at the end".to_owned()));
        }

        if bases.len() != scalars.len() || bases.len() == 0 {
            if !crate::features::in_fuzzing_or_gas_metering() {
                return Err(ApiError::InputError(format!("Multiexp with empty input pairs, file {}, line {}", file!(), line!())));
            } else {
                let result = CurvePoint::zero(&curve);
                return serialize_g2_point_in_fp3(modulus_len, &result);
            }
        } 

        let result = peppinger(&bases, scalars);

        serialize_g2_point_in_fp3(modulus_len, &result)   
    }
}

pub struct PublicG2Api;

impl G2Api for PublicG2Api {
    fn add_points(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (modulus, _, extension_degree, _, _) = parse_modulus_and_extension_degree(&bytes)?;
        let modulus_limbs = num_limbs_for_modulus(&modulus)?;

        let result: Result<Vec<u8>, ApiError> = match extension_degree {
            EXTENSION_DEGREE_2 => {
                let result: Result<Vec<u8>, ApiError> = expand_for_modulus_limbs!(modulus_limbs, G2ApiImplementationFp2, bytes, add_points); 

                result
            },
            EXTENSION_DEGREE_3 => {
                let result: Result<Vec<u8>, ApiError> = expand_for_modulus_limbs!(modulus_limbs, G2ApiImplementationFp3, bytes, add_points); 

                result
            },
            _ => {
                return Err(ApiError::InputError("Invalid extension degree".to_owned()));
            }
        };

        result
    }

    fn mul_point(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (modulus, _, extension_degree, _, _) = parse_modulus_and_extension_degree(&bytes)?;
        let modulus_limbs = num_limbs_for_modulus(&modulus)?;

        let result: Result<Vec<u8>, ApiError> = match extension_degree {
            EXTENSION_DEGREE_2 => {
                let result: Result<Vec<u8>, ApiError> = expand_for_modulus_limbs!(modulus_limbs, G2ApiImplementationFp2, bytes, mul_point); 

                result
            },
            EXTENSION_DEGREE_3 => {
                let result: Result<Vec<u8>, ApiError> = expand_for_modulus_limbs!(modulus_limbs, G2ApiImplementationFp3, bytes, mul_point); 

                result
            },
            _ => {
                return Err(ApiError::InputError("Invalid extension degree".to_owned()));
            }
        };

        result
    }

    fn multiexp(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        let (modulus, _, extension_degree, _, _) = parse_modulus_and_extension_degree(&bytes)?;
        let modulus_limbs = num_limbs_for_modulus(&modulus)?;

        let result: Result<Vec<u8>, ApiError> = match extension_degree {
            EXTENSION_DEGREE_2 => {
                let result: Result<Vec<u8>, ApiError> = expand_for_modulus_limbs!(modulus_limbs, G2ApiImplementationFp2, bytes, multiexp); 

                result
            },
            EXTENSION_DEGREE_3 => {
                let result: Result<Vec<u8>, ApiError> = expand_for_modulus_limbs!(modulus_limbs, G2ApiImplementationFp3, bytes, multiexp); 

                result
            },
            _ => {
                return Err(ApiError::InputError("Invalid extension degree".to_owned()));
            }
        };

        result
    }
}