use axum::{Router, routing::get, response::{Html, Response}};
use axum::http::{header, StatusCode};
use axum::body::Body;

const PAGE:    &str  = include_str!("../static/landing.html");
const ECHARTS: &[u8] = include_bytes!("../static/echarts.min.js");

pub fn router() -> Router {
    Router::new()
        .route("/", get(landing_page))
        .route("/echarts.min.js", get(echarts_js))
}

async fn landing_page() -> Html<&'static str> {
    Html(PAGE)
}

async fn echarts_js() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(ECHARTS))
        .unwrap()
}
