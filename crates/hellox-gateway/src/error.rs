use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use hellox_gateway_api::{ApiError, ErrorEnvelope};

#[derive(Debug)]
pub struct GatewayHttpError {
    status: StatusCode,
    kind: String,
    message: String,
}

impl GatewayHttpError {
    pub(crate) fn internal(message: String) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            kind: "internal_error".to_string(),
            message,
        }
    }

    pub(crate) fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            kind: "invalid_request_error".to_string(),
            message,
        }
    }
}

impl From<anyhow::Error> for GatewayHttpError {
    fn from(error: anyhow::Error) -> Self {
        Self::internal(error.to_string())
    }
}

impl IntoResponse for GatewayHttpError {
    fn into_response(self) -> Response {
        let body = Json(ErrorEnvelope {
            error: ApiError {
                r#type: self.kind,
                message: self.message,
            },
        });

        (self.status, body).into_response()
    }
}
