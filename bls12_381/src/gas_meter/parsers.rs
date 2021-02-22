use crate::public_interface::decode_utils::*;
use crate::public_interface::decode_g1::*;
use crate::public_interface::constants::*;
use crate::errors::ApiError;
use crate::integers::*;


/// return:
/// - modulus, 
/// - scalar field modulus
/// - rest
/// eats up to the operation-specific parameters
pub(crate) fn parse_g1_curve_parameters<'a>(bytes: &'a [u8]) -> Result<(
    MaxFieldUint, 
    usize,
    usize,
    &'a [u8]), ApiError> 
{
    let ((modulus, modulus_len), rest) = get_base_field_params(&bytes)?;
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get A parameter")?;
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get B parameter")?;

    let (order_len, _, rest) = parse_group_order_from_encoding(rest)?;

    if rest.len() == 0 {
        return Err(ApiError::InputError("Input is not long enough".to_owned()));
    }

    Ok(
        (
            modulus,
            modulus_len,
            order_len,
            rest
        )
    )
}

/// return:
/// - modulus, 
/// - scalar field modulus
/// - extension degree
/// - rest
/// eats up to the operation-specific parameters
pub(crate) fn parse_g2_curve_parameters<'a>(bytes: &'a [u8]) -> Result<(
    MaxFieldUint, 
    usize,
    usize,
    u8,
    &'a [u8]), ApiError> 
{
    let ((modulus, modulus_len), rest) = get_base_field_params(&bytes)?;
    let (ext_degree_encoding, rest) = split(&rest, EXTENSION_DEGREE_ENCODING_LENGTH, "Input is not long enough to get extension degree")?;
    let extension_degree = ext_degree_encoding[0];
    if !(extension_degree == EXTENSION_DEGREE_2 || extension_degree == EXTENSION_DEGREE_3) {
        return Err(ApiError::InputError("Invalid extension degree".to_owned()));
    }
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get non-residue")?;
    let extension_field_element_len = modulus_len * (extension_degree as usize);
    let (_, rest) = split(rest, extension_field_element_len, "Input is not long enough to get A parameter")?;
    let (_, rest) = split(rest, extension_field_element_len, "Input is not long enough to get B parameter")?;

    let (order_len, _, rest) = parse_group_order_from_encoding(rest)?;
    if rest.len() == 0 {
        return Err(ApiError::InputError("Input is not long enough".to_owned()));
    }

    Ok(
        (
            modulus,
            modulus_len,
            order_len,
            extension_degree,
            rest
        )
    )
}

pub(crate) fn parse_mnt_pairing_parameters<'a>(bytes: &'a [u8], ext_degree: usize) -> Result<(
    MaxFieldUint, 
    usize,
    usize,
    (u64, u64),
    (u64, u64),
    (u64, u64),
    (usize, usize),
    &'a [u8]), ApiError> 
{
    use crate::public_interface::sane_limits::*;

    let ((modulus, modulus_len), rest) = get_base_field_params(&bytes)?;
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get A parameter")?;
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get B parameter")?;

    let (order_len, _, rest) = parse_group_order_from_encoding(rest)?;

    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get non-residue")?;

    let (x, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_ATE_LOOP_COUNT)?;
    if x.is_zero() {
        return Err(ApiError::InputError("Ate pairing loop count parameters can not be zero".to_owned()));
    }

    let ate_loop_bits = x.bits();
    let ate_loop_hamming = calculate_hamming_weight(&x.as_ref());

    if ate_loop_hamming > MAX_ATE_PAIRING_ATE_LOOP_COUNT_HAMMING {
        return Err(ApiError::InputError("Ate pairing loop has too large hamming weight".to_owned()));
    }

    let (x_sign, rest) = split(rest, SIGN_ENCODING_LENGTH, "Input is not long enough to get X sign encoding")?;
    let _ = match x_sign[0] {
        SIGN_PLUS => false,
        SIGN_MINUS => true,
        _ => {
            return Err(ApiError::InputError("X sign is not encoded properly".to_owned()));
        },
    };

    let (exp_w0, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_FINAL_EXP_W0_BIT_LENGTH)?;
    if exp_w0.is_zero() {
        return Err(ApiError::InputError("Final exp w0 loop count parameters can not be zero".to_owned()));
    }
    let exp_w0_bits = exp_w0.bits();
    let exp_w0_hamming = calculate_hamming_weight(&exp_w0.as_ref());

    let (exp_w1, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, MAX_ATE_PAIRING_FINAL_EXP_W1_BIT_LENGTH)?;
    if exp_w1.is_zero() {
        return Err(ApiError::InputError("Final exp w1 loop count parameters can not be zero".to_owned()));
    }
    let exp_w1_bits = exp_w1.bits();
    let exp_w1_hamming = calculate_hamming_weight(&exp_w1.as_ref());

    let (exp_w0_sign, rest) = split(rest, SIGN_ENCODING_LENGTH, "Input is not long enough to get exp_w0 sign encoding")?;
    let _ = match exp_w0_sign[0] {
        SIGN_PLUS => false,
        SIGN_MINUS => true,
        _ => {
            return Err(ApiError::InputError("Exp_w0 sign is not encoded properly".to_owned()));
        },
    };

    let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
    let num_pairs = num_pairs_encoding[0] as usize;

    if num_pairs == 0 {
        return Err(ApiError::InputError("Zero pairs encoded".to_owned()));
    }
    
    let mut num_g1_subgroup_checks = 0;
    let mut num_g2_subgroup_checks = 0;

    let mut grobal_rest = rest;

    if num_pairs == 0 {
        return Err(ApiError::InputError("Zero pairs encoded".to_owned()));
    }

    for _ in 0..num_pairs {
        let (check_g1, rest) = decode_boolean(&grobal_rest)?;
        let (_, rest) = split(rest, modulus_len*2, "input is not long enough to get G1 point encoding")?;
        let (check_g2, rest) = decode_boolean(&rest)?;
        let (_, rest) = split(rest, modulus_len*2*ext_degree, "input is not long enough to get G2 point encoding")?;
        grobal_rest = rest;

        if check_g1 {
            num_g1_subgroup_checks += 1;
        }

        if check_g2 {
            num_g2_subgroup_checks += 1;
        }
    }

    if grobal_rest.len() != 0 {
        return Err(ApiError::InputError("Input has garbage at the end for MNT4/6 pairing".to_owned()));
    }

    Ok(
        (
            modulus,
            order_len,
            num_pairs,
            (ate_loop_bits as u64, ate_loop_hamming as u64),
            (exp_w0_bits as u64, exp_w0_hamming as u64),
            (exp_w1_bits as u64, exp_w1_hamming as u64),
            (num_g1_subgroup_checks, num_g2_subgroup_checks),
            rest
        )
    )
}

pub(crate) fn parse_bls12_bn_pairing_parameters<'a>(bytes: &'a [u8], max_x_bit_limit: usize) -> Result<(
    MaxFieldUint, 
    usize,
    usize,
    MaxLoopParametersUint,
    bool,
    (usize, usize),
    &'a [u8]), ApiError> 
{
    use crate::pairings::TwistType;

    let ((modulus, modulus_len), rest) = get_base_field_params(&bytes)?;
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get A parameter")?;
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get B parameter")?;

    let (order_len, _, rest) = parse_group_order_from_encoding(rest)?;
    
    let (_, rest) = split(rest, modulus_len, "Input is not long enough to get Fp2 non-residue")?;
    let (_, rest) = split(rest, modulus_len*2, "Input is not long enough to get Fp6/Fp12 non-residue")?;

    let (twist_type_encoding, rest) = split(rest, TWIST_TYPE_LENGTH, "Input is not long enough to get twist type")?;

    let _ = match twist_type_encoding[0] {
        TWIST_TYPE_D => TwistType::D,
        TWIST_TYPE_M => TwistType::M, 
        _ => {
            return Err(ApiError::UnknownParameter("Unknown twist type supplied".to_owned()));
        },
    };

    let (x, rest) = decode_loop_parameter_scalar_with_bit_limit(&rest, max_x_bit_limit)?;
    if x.is_zero() {
        return Err(ApiError::InputError("Ate pairing loop count parameters can not be zero".to_owned()));
    }

    let (x_sign, rest) = split(rest, SIGN_ENCODING_LENGTH, "Input is not long enough to get X sign encoding")?;
    let x_is_negative = match x_sign[0] {
        SIGN_PLUS => false,
        SIGN_MINUS => true,
        _ => {
            return Err(ApiError::InputError("X sign is not encoded properly".to_owned()));
        },
    };

    let (num_pairs_encoding, rest) = split(rest, BYTES_FOR_LENGTH_ENCODING, "Input is not long enough to get number of pairs")?;
    let num_pairs = num_pairs_encoding[0] as usize;

    let mut num_g1_subgroup_checks = 0;
    let mut num_g2_subgroup_checks = 0;

    let mut grobal_rest = rest;

    if num_pairs == 0 {
        return Err(ApiError::InputError("Zero pairs encoded".to_owned()));
    }

    for _ in 0..num_pairs {
        let (check_g1, rest) = decode_boolean(&grobal_rest)?;
        let (_, rest) = split(rest, modulus_len*2, "input is not long enough to get G1 point encoding")?;
        let (check_g2, rest) = decode_boolean(&rest)?;
        let (_, rest) = split(rest, modulus_len*4, "input is not long enough to get G2 point encoding")?;
        grobal_rest = rest;

        if check_g1 {
            num_g1_subgroup_checks += 1;
        }

        if check_g2 {
            num_g2_subgroup_checks += 1;
        }
    }

    if grobal_rest.len() != 0 {
        return Err(ApiError::InputError("Input has garbage at the end for BLS12/BN pairing".to_owned()));
    }

    Ok(
        (
            modulus,
            order_len,
            num_pairs,
            x,
            x_is_negative,
            (num_g1_subgroup_checks, num_g2_subgroup_checks),
            rest
        )
    )
}

use serde::{Deserializer};
use std::collections::HashMap;

pub(crate) fn parse_hashmap_usize_u64_from_ints<'de, D>(deserializer: D) -> Result<HashMap<usize, u64>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;
    use serde::de::{Visitor, SeqAccess};

    struct MyVisitor;

    impl<'de> Visitor<'de> for MyVisitor
    {
        type Value = HashMap<usize, u64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a nonempty sequence of numbers")
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            let mut results = HashMap::with_capacity(seq.size_hint().unwrap_or(100));
            while let Some(value) = seq.next_element::<[Value; 2]>()? {
                let first = value[0].as_u64().expect(&format!("should be an integer {:?}", value[0])) as usize;
                let second = value[1].as_u64().expect(&format!("should be an integer {:?}", value[1]));
                results.insert(first, second);
            }

            Ok(results)
        }
    }

    let visitor = MyVisitor;
    let result = deserializer.deserialize_seq(visitor)?;

    Ok(result)
}