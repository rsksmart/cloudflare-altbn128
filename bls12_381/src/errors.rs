use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    Overflow,
    UnexpectedZero(String),
    InputError(String),
    DivisionByZero,
    UnknownParameter(String),
    OutputError(String),
    MissingValue,
}

impl Error for ApiError {
    fn description(&self) -> &str {
        match *self {
            ApiError::Overflow => "overflow",
            ApiError::UnexpectedZero(_) => "parameter expected to be non-zero",
            ApiError::InputError(_) => "invalid input parameters",
            ApiError::DivisionByZero => "division by zero",
            ApiError::UnknownParameter(_) => "parameter has value out of bounds",
            ApiError::OutputError(_) => "error outputing results",
            ApiError::MissingValue => "missing value",
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            ApiError::Overflow => write!(f, "overflow"),
            ApiError::UnexpectedZero(descr) => write!(f, "parameter expected to be non-zero, {}", descr),
            ApiError::InputError(descr) => write!(f, "invalid input parameters, {}", descr),
            ApiError::DivisionByZero => write!(f, "division by zero"),
            ApiError::UnknownParameter(descr) => write!(f, "parameter has value out of bounds, {}", descr),
            ApiError::OutputError(descr) => write!(f, "error outputing results, {}", descr),
            ApiError::MissingValue => write!(f, "missing value"),
        }
    }
}