use axum::{Router, routing::get};

async fn handle_health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(handle_health));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
