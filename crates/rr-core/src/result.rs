use crate::CoreError;

pub type CoreResult<T> = std::result::Result<T, CoreError>;
