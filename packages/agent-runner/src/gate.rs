// Gate layer: auth middleware.
// Phase 2: log-only pass-through. Phase 3: mTLS + signed JWT.
// Wired into the stack via axum::middleware::from_fn in main.rs.

use axum::{http::Request, middleware::Next, response::Response, body::Body};

pub async fn require_auth(req: Request<Body>, next: Next) -> Response {
    // Future: extract + validate Bearer token here
    next.run(req).await
}
