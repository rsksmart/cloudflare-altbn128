use crate::public_interface::constants::*;
use crate::public_interface::{PublicG1Api, G1Api, PublicG2Api, G2Api};
use crate::errors::ApiError;

use num_bigint::BigUint;

use crate::test::parsers::*;
use super::call_pairing_engine;

use crate::test::g1_ops;
use crate::test::g2_ops;

pub(crate) fn assemble_single_curve_params(curve: JsonBnPairingCurveParameters, pairs: usize, check_subgroup: bool) -> Result<Vec<u8>, ApiError> {
    let curve_clone = curve.clone();
    assert!(pairs % 2 == 0);

    // - Curve type
    // - Lengths of modulus (in bytes)
    // - Field modulus
    // - Curve A
    // - Curve B
    // - non-residue for Fp2
    // - non-residue for Fp6
    // - twist type M/D
    // - parameter X
    // - sign of X
    // - number of pairs
    // - list of encoded pairs

    // first determine the length of the modulus
    let modulus = curve.q;
    let modulus_length = modulus.clone().to_bytes_be().len();

    let curve_type = vec![BN];
    let modulus_len_encoded = vec![modulus_length as u8];
    let modulus_encoded = pad_for_len_be(modulus.clone().to_bytes_be(), modulus_length);

    let a_encoded = apply_sign(curve.a, &modulus);
    assert!(a_encoded < modulus);
    let a_encoded = pad_for_len_be(a_encoded.to_bytes_be(), modulus_length);

    let b_encoded = apply_sign(curve.b, &modulus);
    assert!(b_encoded < modulus);
    let b_encoded = pad_for_len_be(b_encoded.to_bytes_be(), modulus_length);

    let fp2_nonres_encoded = {
        let (mut nonres, is_positive) = curve.non_residue;
        if !is_positive {
            nonres = modulus.clone() - nonres;
        }
        pad_for_len_be(nonres.to_bytes_be(), modulus_length)
    };

    let fp6_nonres_encoded_c0 = {
        let (mut nonres, is_positive) = curve.quadratic_non_residue_0;
        if !is_positive {
            nonres = modulus.clone() - nonres;
        }
        pad_for_len_be(nonres.to_bytes_be(), modulus_length)
    };

    let fp6_nonres_encoded_c1 = {
        let (mut nonres, is_positive) = curve.quadratic_non_residue_1;
        if !is_positive {
            nonres = modulus.clone() - nonres;
        }
        pad_for_len_be(nonres.to_bytes_be(), modulus_length)
    };

    let twist_type = if curve.is_d_type { vec![TWIST_TYPE_D] } else { vec![TWIST_TYPE_M] };

    let (x_decoded, x_is_positive) = curve.x;
    let x_sign = if x_is_positive { vec![0u8] } else { vec![1u8] };
    let x_encoded = x_decoded.to_bytes_be();
    let x_length = vec![x_encoded.len() as u8];

    // now we make two random scalars and do scalar multiplications in G1 and G2 to get pairs that should
    // at the end of the day pair to identity element

    let group_size = curve.r;
    let group_size_encoded = group_size.clone().to_bytes_be();
    let group_size_length = group_size_encoded.len();
    let group_len_encoded = vec![group_size_length as u8];

    // first parse generators
    // g1 generator
    let g1_x = curve.g1_x;
    let g1_x = pad_for_len_be(g1_x.to_bytes_be(), modulus_length);
    let g1_y = curve.g1_y;
    let g1_y = pad_for_len_be(g1_y.to_bytes_be(), modulus_length);

    // g2 generator
    let g2_x_0 = curve.g2_x_0;
    let g2_x_1 = curve.g2_x_1;

    let g2_y_0 = curve.g2_y_0;
    let g2_y_1 = curve.g2_y_1;

    let num_pairs = vec![pairs as u8];

    let mut g1_encodings = vec![];
    let mut g2_encodings = vec![];

    let g2_generator_encoding = {
        let mut g2_generator_encoding = vec![];
        g2_generator_encoding.extend(pad_for_len_be(g2_x_0.to_bytes_be(), modulus_length));
        g2_generator_encoding.extend(pad_for_len_be(g2_x_1.to_bytes_be(), modulus_length));
        g2_generator_encoding.extend(pad_for_len_be(g2_y_0.to_bytes_be(), modulus_length));
        g2_generator_encoding.extend(pad_for_len_be(g2_y_1.to_bytes_be(), modulus_length));

        g2_generator_encoding
    };

    // for multiplications we use the public API itself - just construct the corresponding G1
    // multiplication API. Leave G2 as generators for now

    use rand::{Rng, SeedableRng};
    use rand_xorshift::XorShiftRng;

    let rng = &mut XorShiftRng::from_seed([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);

    {
        fn make_random_scalar<R: Rng>(rng: &mut R, group_size_length: usize, group_size: &BigUint) -> BigUint {
            let random_scalar_bytes: Vec<u8> = (0..group_size_length).map(|_| rng.gen()).collect();
            let random_scalar = BigUint::from_bytes_be(&random_scalar_bytes[..]);
            let random_scalar = random_scalar % group_size;

            random_scalar
        }

        for _ in 0..(pairs/2) {
            // - Multiplication API signature
            // - Lengths of modulus (in bytes)
            // - Field modulus
            // - Curve A
            // - Curve B
            // - Length of a scalar field (curve order) (in bytes)
            // - Curve order
            // - X
            // - Y
            // - Scalar
            
            let r1 = make_random_scalar(rng, group_size_length, &group_size);
            let r2 = make_random_scalar(rng, group_size_length, &group_size);
            let r3 = (r1.clone() * &r2) % &group_size;
            let r3 = group_size.clone() - r3;

            // pair (g1^r1, g2^r2)*(g1^(-r1*r2), g2)
            let (g1_common_bytes, _, _) = g1_ops::bn::assemble_single_curve_params(curve_clone.clone());
            let (g2_common_bytes, _, _) = g2_ops::bn::assemble_single_curve_params(curve_clone.clone());

            let g1_encoded_0 = {
                let mut mul_calldata = vec![];
                mul_calldata.extend(g1_common_bytes.clone());
                mul_calldata.extend_from_slice(&g1_x[..]);
                mul_calldata.extend_from_slice(&g1_y[..]);
                mul_calldata.extend(pad_for_len_be(r1.to_bytes_be(), group_size_length));

                let g1 = PublicG1Api::mul_point(&mul_calldata[..])?;

                g1
            };

            let g2_encoded_0 = {
                let mut mul_calldata = vec![];
                mul_calldata.extend(g2_common_bytes.clone());
                mul_calldata.extend(g2_generator_encoding.clone());
                mul_calldata.extend(pad_for_len_be(r2.to_bytes_be(), group_size_length));

                let g2 = PublicG2Api::mul_point(&mul_calldata[..])?;

                g2
            };

            let g1_encoded_1 = {
                let mut mul_calldata = vec![];
                mul_calldata.extend(g1_common_bytes.clone());
                mul_calldata.extend_from_slice(&g1_x[..]);
                mul_calldata.extend_from_slice(&g1_y[..]);
                mul_calldata.extend(pad_for_len_be(r3.to_bytes_be(), group_size_length));

                let g1 = PublicG1Api::mul_point(&mul_calldata[..])?;

                g1
            };

            let g2_encoded_1 = g2_generator_encoding.clone();
            g1_encodings.push(g1_encoded_0);
            g1_encodings.push(g1_encoded_1);

            g2_encodings.push(g2_encoded_0);
            g2_encodings.push(g2_encoded_1);
        }
    }

    let mut calldata = vec![];
    calldata.extend(curve_type.into_iter());
    calldata.extend(modulus_len_encoded.into_iter());
    calldata.extend(modulus_encoded.into_iter());
    calldata.extend(a_encoded.into_iter());
    calldata.extend(b_encoded.into_iter());
    calldata.extend(group_len_encoded.into_iter());
    calldata.extend(group_size_encoded.into_iter());
    calldata.extend(fp2_nonres_encoded.into_iter());
    calldata.extend(fp6_nonres_encoded_c0.into_iter());
    calldata.extend(fp6_nonres_encoded_c1.into_iter());
    calldata.extend(twist_type.into_iter());
    calldata.extend(x_length.into_iter());
    calldata.extend(x_encoded.into_iter());
    calldata.extend(x_sign.into_iter());
    calldata.extend(num_pairs.into_iter());
    for (g1, g2) in g1_encodings.into_iter().zip(g2_encodings.into_iter()) {
        if check_subgroup {
            calldata.extend(vec![1u8]);
        } else {
            calldata.extend(vec![0u8]);
        }
        calldata.extend(g1.into_iter());
        if check_subgroup {
            calldata.extend(vec![1u8]);
        } else {
            calldata.extend(vec![0u8]);
        }
        calldata.extend(g2.into_iter());
    }

    Ok(calldata)
}

// #[test]
// fn test_single() {
//     let calldata = assemble_single();
//     let result = call_pairing_engine(&calldata[..]);
//     assert!(result.is_ok());

//     let result = result.unwrap()[0];
//     assert!(result == 1u8);
// }

#[test]
fn test_bn_pairings_from_vectors() {
    // let curves = read_dir_and_grab_curves::<JsonBnPairingCurveParameters>("src/test/test_vectors/bn/negative_u/");
    let curves = read_dir_and_grab_curves::<JsonBnPairingCurveParameters>("src/test/test_vectors/bn/");
    assert!(curves.len() != 0);
    for (curve, file_name) in curves.into_iter() {
        let u_is_positive = curve.x.1;
        let calldata = assemble_single_curve_params(curve, 2, true).unwrap();
        let result = call_pairing_engine(&calldata[..]);
        if !result.is_ok() {
            println!("Failed on {} with result {}", file_name, result.err().unwrap());
            continue;
        }
        // assert!(result.is_ok(), "Failed on {}", file_name);
        println!("U is positive = {}", u_is_positive);

        let result = result.unwrap()[0];
        if result != 1u8 {
            println!("Failed on {}", file_name);
        }
        // assert!(result == 1u8, "Failed on {}", file_name);
    }
}

extern crate hex;
extern crate csv;

use hex::{encode};
use csv::{Writer};

#[test]
#[ignore]
fn dump_pairing_vectors() {
    let curves = read_dir_and_grab_curves::<JsonBnPairingCurveParameters>("src/test/test_vectors/bn/");
    assert!(curves.len() != 0);
    let mut writer = Writer::from_path("src/test/test_vectors/bn/pairing.csv").expect("must open a test file");
    writer.write_record(&["input", "result"]).expect("must write header");
    for (curve, _) in curves.into_iter() {
        let mut input_data = vec![OPERATION_PAIRING];
        let calldata = assemble_single_curve_params(curve.clone(), 2, true).unwrap();
        input_data.extend(calldata);
        let expected_result = vec![1u8];
        writer.write_record(&[
            prepend_0x(&encode(&input_data[..])), 
            prepend_0x(&encode(&expected_result[..]))],
        ).expect("must write a record");
    }
    writer.flush().expect("must finalize writing");
}

#[test]
#[ignore]
fn dump_fuzzing_vectors() {
    use std::io::Write;
    use std::fs::File;
    let curves = read_dir_and_grab_curves::<JsonBnPairingCurveParameters>("src/test/test_vectors/bn/");
    assert!(curves.len() != 0);
    
    // let mut writer = Writer::from_path("src/test/test_vectors/bls12/pairing.csv").expect("must open a test file");
    // writer.write_record(&["input", "result"]).expect("must write header");
    for (curve, _) in curves.into_iter() {
        let mut input_data = vec![OPERATION_PAIRING];
        let calldata = assemble_single_curve_params(curve.clone(), 2, true).unwrap();
        input_data.extend(calldata);
        let filename = hex::encode(&input_data);
        let mut f = File::create(&format!("src/test/test_vectors/bn/fuzzing_corpus/{}", &filename[0..40])).unwrap();
        f.write_all(&mut input_data[..]).expect("must write");
    }
}

// use rust_test::Bencher;

// #[bench]
// fn bench_single(b: &mut Bencher) {
//     let calldata = assemble_single();
//     b.iter(|| {
//         call_pairing_engine(&calldata[..]).expect("must use");
//     });
// }

pub(crate) fn assemble_bn254(num_point_pairs: usize) -> Vec<u8> {
    /// - Curve type
    /// - Lengths of modulus (in bytes)
    /// - Field modulus
    /// - Curve A
    /// - Curve B
    // - non-residue for Fp2
    // - non-residue for Fp6
    // - twist type M/D
    // - parameter X
    // - sign of X
    // - number of pairs
    // - list of encoded pairs
    use num_traits::Num;
    let modulus_length = 32;
    let modulus = BigUint::from_str_radix("21888242871839275222246405745257275088696311157297823662689037894645226208583", 10).unwrap();
    let curve_type = vec![BN];
    let modulus_len_encoded = vec![modulus_length as u8];
    let modulus_encoded = pad_for_len_be(modulus.clone().to_bytes_be(), modulus_length);
    let a_encoded = pad_for_len_be(BigUint::from(0u64).to_bytes_be(), modulus_length);
    let b_encoded = pad_for_len_be(BigUint::from(3u64).to_bytes_be(), modulus_length);
    let group_order_len = 32;
    let group_order = BigUint::from_str_radix("21888242871839275222246405745257275088548364400416034343698204186575808495617", 10).unwrap();
    let group_order_encoding = pad_for_len_be(group_order.to_bytes_be(), group_order_len);
    let minus_one = modulus.clone() - BigUint::from(1u64);
    let fp2_nonres_encoded = pad_for_len_be(minus_one.to_bytes_be(), modulus_length);
    let fp6_nonres_encoded_c0 = pad_for_len_be(BigUint::from(9u64).to_bytes_be(), modulus_length);
    let fp6_nonres_encoded_c1 = pad_for_len_be(BigUint::from(1u64).to_bytes_be(), modulus_length);
    let twist_type = vec![TWIST_TYPE_D];
    let x_length = vec![8u8];
    let x_encoded = pad_for_len_be(BigUint::from(4965661367192848881 as u64).to_bytes_be(), 8); 
    let x_sign = vec![SIGN_PLUS];
    let num_pairs = vec![num_point_pairs as u8];
    // first pair
    let p_x = BigUint::from_str_radix("1", 10).unwrap().to_bytes_be();
    let p_y = BigUint::from_str_radix("2", 10).unwrap().to_bytes_be();
    let mut g1_0_encoding: Vec<u8> = vec![];
    g1_0_encoding.push(1u8);
    g1_0_encoding.extend(pad_for_len_be(p_x.clone(), modulus_length).into_iter());
    g1_0_encoding.extend(pad_for_len_be(p_y, modulus_length).into_iter());

    let q_x_0 = BigUint::from_str_radix("10857046999023057135944570762232829481370756359578518086990519993285655852781", 10).unwrap().to_bytes_be();
    let q_x_1 = BigUint::from_str_radix("11559732032986387107991004021392285783925812861821192530917403151452391805634", 10).unwrap().to_bytes_be();
    let q_y_0 = BigUint::from_str_radix("8495653923123431417604973247489272438418190587263600148770280649306958101930", 10).unwrap().to_bytes_be();
    let q_y_1 = BigUint::from_str_radix("4082367875863433681332203403145435568316851327593401208105741076214120093531", 10).unwrap().to_bytes_be();

    let mut g2_0_encoding = vec![];
    g2_0_encoding.push(1u8);
    g2_0_encoding.extend(pad_for_len_be(q_x_0.clone(), modulus_length).into_iter());
    g2_0_encoding.extend(pad_for_len_be(q_x_1.clone(), modulus_length).into_iter());
    g2_0_encoding.extend(pad_for_len_be(q_y_0.clone(), modulus_length).into_iter());
    g2_0_encoding.extend(pad_for_len_be(q_y_1.clone(), modulus_length).into_iter());

    // second pair 
    let y = modulus.clone() - BigUint::from_str_radix("2", 10).unwrap();

    let mut g1_1_encoding: Vec<u8> = vec![];
    g1_1_encoding.push(1u8);
    g1_1_encoding.extend(pad_for_len_be(p_x.clone(), modulus_length).into_iter());
    g1_1_encoding.extend(pad_for_len_be(y.to_bytes_be(), modulus_length).into_iter());

    let g2_1_encoding = g2_0_encoding.clone();

    let mut calldata = vec![];
    calldata.extend(curve_type.into_iter());
    calldata.extend(modulus_len_encoded.into_iter());
    calldata.extend(modulus_encoded.into_iter());
    calldata.extend(a_encoded.into_iter());
    calldata.extend(b_encoded.into_iter());
    calldata.extend(vec![group_order_len as u8]);
    calldata.extend(group_order_encoding.into_iter());
    calldata.extend(fp2_nonres_encoded.into_iter());
    calldata.extend(fp6_nonres_encoded_c0.into_iter());
    calldata.extend(fp6_nonres_encoded_c1.into_iter());
    calldata.extend(twist_type.into_iter());
    calldata.extend(x_length.into_iter());
    calldata.extend(x_encoded.into_iter());
    calldata.extend(x_sign.into_iter());
    calldata.extend(num_pairs.into_iter());

    for i in 0..num_point_pairs {
        if i % 2 == 0 {
            calldata.extend(g1_0_encoding.clone().into_iter());
            calldata.extend(g2_0_encoding.clone().into_iter());
        } else {
            calldata.extend(g1_1_encoding.clone().into_iter());
            calldata.extend(g2_1_encoding.clone().into_iter());
        }
    }

    calldata
}

#[test]
fn test_call_public_api_on_bn254() {
    let calldata = assemble_bn254(4);
    use crate::public_interface::PairingApi;

    let result = crate::public_interface::PublicPairingApi::pair(&calldata).unwrap();
    assert!(result.len() == 1);
    assert!(result[0] == 1);
}

fn strip_0x(string: &str) -> String {
    let string = string.trim();
    let mut string = string.to_ascii_lowercase().as_bytes().to_vec();
    if string.len() > 2 && string[0..1] == b"0"[..] && string[1..2] == b"x"[..] {
        string = string[2..].to_vec();
    }
    
    std::string::String::from_utf8(string).unwrap()
}

fn strip_0x_and_get_sign(string: &str) -> (String, bool) {
    let string = string.trim();
    let mut string = string.to_ascii_lowercase().as_bytes().to_vec();
    let mut positive = true;
    if string.len() > 1 && string[0..1] == b"-"[..] {
        string = string[1..].to_vec();
        positive = false;
    }
    if string.len() > 1 && string[0..1] == b"+"[..] {
        string = string[1..].to_vec();
    }
    if string.len() > 2 && string[0..1] == b"0"[..] && string[1..2] == b"x"[..] {
        string = string[2..].to_vec();
    }
    
    (std::string::String::from_utf8(string).unwrap(), positive)
}

fn strip_0x_and_pad(string: &str) -> String {
    let string = string.trim();
    let mut string = string.to_ascii_lowercase().as_bytes().to_vec();
    if string.len() > 2 && string[0..1] == b"0"[..] && string[1..2] == b"x"[..] {
        let mut string = string[2..].to_vec();
        if string.len() % 2 == 1 {
            string = {
                let mut res = "0".as_bytes().to_vec();
                res.extend(string.into_iter());

                res
            };
        }
        return std::string::String::from_utf8(string).unwrap();
    }
    if string.len() % 2 == 1 {
        string = {
            let mut res = "0".as_bytes().to_vec();
            res.extend(string.into_iter());

            res
        };
    }

    std::string::String::from_utf8(string).unwrap()
}