use axum::{Router, routing::get, routing::post};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod ast;
mod executor;
mod models;
mod routes;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app = Router::new()
        .route("/pipeline", post(routes::pipeline::handle_pipeline))
        .route("/health", get(|| async { "ok" }))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], 6778));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind to address {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    info!("sandbox-service listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
