use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("no authenticated session is available")]
    NoSessionAvailable,
    #[error("session store unavailable: {0}")]
    SessionStoreUnavailable(String),
    #[error("invalid cached headers for {username}: {reason}")]
    InvalidCachedHeaders { username: String, reason: String },
    #[error("missing customer id in cached session for {username}")]
    MissingCustomerId { username: String },
    #[error("RCI authentication failed with status {status}")]
    RciAuthFailed { status: u16 },
    #[error("RCI unavailable: {0}")]
    RciUnavailable(String),
    #[error("RCI returned an unexpected status {status}")]
    RciUnexpectedStatus { status: u16 },
}
