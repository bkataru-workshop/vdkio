use std::num::ParseIntError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VdkError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("codec error: {0}")]
    Codec(String),
    
    #[error("protocol error: {0}")]
    Protocol(String),
    
    #[error("parser error: {0}")]
    Parser(String),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("parse int error: {0}")]
    ParseInt(#[from] ParseIntError),
}

pub type Result<T> = std::result::Result<T, VdkError>;
