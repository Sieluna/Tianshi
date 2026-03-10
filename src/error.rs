use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Buffer length must be multiple of 4 (f32 size)")]
    InvalidPointCloudLength,
}

pub type Result<T> = core::result::Result<T, Error>;
