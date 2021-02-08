use crate::manager::ControlContext;
use std::sync::Arc;
use thiserror::Error;
use warp::Reply;
use warp::{reply::Response, Rejection};
/// Header where token should be present in requests
pub const API_TOKEN_HEADER: &str = "API-Token";

/// API Token wrapper type
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct APIToken(Vec<u8>);

impl APIToken {
    pub fn from_string(token: String) -> Result<Self, TokenError> {
        let mut token_vec: Vec<u8> = Vec::new();
        base64::decode_config_buf(token, base64::URL_SAFE, &mut token_vec)?;
        Ok(Self(token_vec))
    }
}

pub struct APITokenManager {
    verification_token: APIToken,
}

impl From<&[u8]> for APIToken {
    fn from(data: &[u8]) -> Self {
        Self(data.to_vec())
    }
}

impl AsRef<[u8]> for APIToken {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl APIToken {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }
}

impl APITokenManager {
    fn new(token: String) -> Result<Self, TokenError> {
        Ok(Self {
            verification_token: APIToken::from_string(token)?,
        })
    }

    pub fn is_token_valid(&self, token: APIToken) -> bool {
        self.verification_token == token
    }
}

pub async fn authorize_token(
    token: String,
    context: Arc<std::sync::Mutex<ControlContext>>,
) -> Result<(), Rejection> {
    let api_token = APIToken::from_string(token).map_err(warp::reject::custom)?;

    if context.lock().unwrap().api_token().is_none() {
        return Ok(());
    }

    let manager = APITokenManager::new(context.lock().unwrap().api_token().unwrap())
        .map_err(warp::reject::custom)?;

    if !manager.is_token_valid(api_token) {
        return Err(warp::reject::custom(TokenError::UnauthorizedToken));
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("cannot parse token")]
    ParseError(#[from] base64::DecodeError),
    #[error("invalid token {0}:{1}")]
    InvalidHeader(String, String),
    #[error("unauthorized")]
    UnauthorizedToken,
}

impl warp::reject::Reject for TokenError {}

impl TokenError {
    fn to_response(&self) -> Response {
        let status_code = self.to_status_code();
        warp::reply::with_status(warp::reply::json(&self.to_json()), status_code).into_response()
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"code": self.to_status_code().as_u16(), "message" : self.to_message()})
    }

    fn to_message(&self) -> String {
        format!("{}", self)
    }

    fn to_status_code(&self) -> warp::http::StatusCode {
        match self {
            Self::ParseError(_) => warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            Self::UnauthorizedToken => warp::http::StatusCode::UNAUTHORIZED,
            Self::InvalidHeader(_, _) => warp::http::StatusCode::BAD_REQUEST,
        }
    }
}

impl warp::Reply for TokenError {
    fn into_response(self) -> Response {
        self.to_response()
    }
}
