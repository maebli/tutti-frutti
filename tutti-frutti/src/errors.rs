use reqwest::Error as ReqwestError;
use std::error::Error;
use std::fmt;
use tokio::time::error::Elapsed;

/// Represents errors that can occur while fetching listings.
#[derive(Debug)]
pub enum FetchListingsError {
    /// An error occurred during an HTTP request.
    RequestError(ReqwestError),
    /// The request timed out.
    TimeoutError,
    /// An error occurred while retrieving the CSRF token.
    CsrfTokenError(String),
    /// An error occurred while parsing a response.
    ParseError(String),
}

impl fmt::Display for FetchListingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchListingsError::RequestError(e) => write!(f, "Request error: {}", e),
            FetchListingsError::TimeoutError => write!(f, "Request timed out"),
            FetchListingsError::CsrfTokenError(msg) => write!(f, "CSRF token error: {}", msg),
            FetchListingsError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl Error for FetchListingsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FetchListingsError::RequestError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ReqwestError> for FetchListingsError {
    fn from(err: ReqwestError) -> FetchListingsError {
        FetchListingsError::RequestError(err)
    }
}

impl From<Elapsed> for FetchListingsError {
    fn from(_: Elapsed) -> FetchListingsError {
        FetchListingsError::TimeoutError
    }
}
