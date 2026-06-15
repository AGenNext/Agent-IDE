use axum::{Router, routing::get, response::Html};

const PAGE: &str = include_str!("../static/landing.html");

pub fn router() -> Router {
    Router::new().route("/", get(landing_page))
}

async fn landing_page() -> Html<&'static str> {
    Html(PAGE)
}
