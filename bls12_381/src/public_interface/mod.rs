pub(crate) mod decode_g1;
pub(crate) mod decode_g2;
pub(crate) mod decode_fp;
pub(crate) mod decode_utils;

#[macro_use]
pub(crate) mod api_specialization_macro;

mod g1_ops;
mod g2_ops;
mod pairing_ops;

pub mod sane_limits;
pub mod constants;

pub use pairing_ops::{PairingApi, PublicPairingApi};
pub use g1_ops::{G1Api, PublicG1Api};
pub use g2_ops::{G2Api, PublicG2Api};

mod unified_api;
pub use self::unified_api::{OperationType, perform_operation, PREALLOCATE_FOR_ERROR_BYTES, PREALLOCATE_FOR_RESULT_BYTES};
pub use crate::errors::ApiError;

#[cfg(feature = "c_api")]
mod c_api;
#[cfg(feature = "c_api")]
pub use self::c_api::{c_perform_operation};

#[cfg(feature = "eip_2537")]
pub mod eip2537;

pub struct API;

impl API {
    pub fn run(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
        use decode_utils::split;
        use constants::*;

        let (op_type, rest) = split(bytes, OPERATION_ENCODING_LENGTH , "Input should be longer than operation type encoding")?;

        match op_type[0] {
            OPERATION_G1_ADD => {
                PublicG1Api::add_points(&rest)
            },
            OPERATION_G1_MUL => {
                PublicG1Api::mul_point(&rest)
            },
            OPERATION_G1_MULTIEXP => {
                PublicG1Api::multiexp(&rest)
            },
            OPERATION_G2_ADD => {
                PublicG2Api::add_points(&rest)
            },
            OPERATION_G2_MUL => {
                PublicG2Api::mul_point(&rest)
            },
            OPERATION_G2_MULTIEXP => {
                PublicG2Api::multiexp(&rest)
            },
            OPERATION_PAIRING => {
                PublicPairingApi::pair(&rest)
            },
            _ => {
                return Err(ApiError::InputError("Unknown operation type".to_owned()));
            }
        }
    }
}