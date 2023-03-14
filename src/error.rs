use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum AppError {
    #[error("")]
    Error(String),
}
