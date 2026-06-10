use crate::ReportError;

pub type ReportResult<T> = std::result::Result<T, ReportError>;
