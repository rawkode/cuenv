//! HTTP response helpers and reporting utilities
//!
//! Provides utilities for creating consistent HTTP responses
//! with proper headers and status codes.

use hyper::{Body, Response, StatusCode};

/// HTTP response helper for creating consistent responses
pub struct HttpResponse;

impl HttpResponse {
    /// Create a successful JSON response
    pub fn ok_json(
        body: impl serde::Serialize,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        let json = match serde_json::to_string(&body) {
            Ok(json) => json,
            Err(e) => return Err(Box::new(e)),
        };

        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Cache-Control", "no-cache, no-store, must-revalidate")
            .body(Body::from(json));

        match response {
            Ok(resp) => Ok(resp),
            Err(e) => Err(Box::new(e)),
        }
    }

    /// Create a successful text response
    pub fn ok_text(
        body: String,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/plain")
            .header("Cache-Control", "no-cache, no-store, must-revalidate")
            .body(Body::from(body));

        match response {
            Ok(resp) => Ok(resp),
            Err(e) => Err(Box::new(e)),
        }
    }

    /// Create an error response with status and message
    pub fn error(status: StatusCode, message: String) -> Response<Body> {
        Response::builder()
            .status(status)
            .header("Content-Type", "text/plain")
            .body(Body::from(message))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Internal server error"))
                    .unwrap()
            })
    }

    /// Create a 404 Not Found response
    pub fn not_found() -> Response<Body> {
        Self::error(StatusCode::NOT_FOUND, "Not Found".to_string())
    }

    /// Create a 401 Unauthorized response
    pub fn unauthorized() -> Response<Body> {
        Self::error(StatusCode::UNAUTHORIZED, "Unauthorized".to_string())
    }

    /// Create a 429 Too Many Requests response
    pub fn rate_limited() -> Response<Body> {
        Self::error(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded".to_string(),
        )
    }

    /// Create a 500 Internal Server Error response
    pub fn internal_error(message: String) -> Response<Body> {
        Self::error(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    /// Create an HTML response
    pub fn ok_html(
        body: String,
    ) -> Result<Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(Body::from(body));

        match response {
            Ok(resp) => Ok(resp),
            Err(e) => Err(Box::new(e)),
        }
    }
}
