use axum::{Router, routing::post};
use std::net::SocketAddr;

mod ast;
mod error;
mod executor;
mod models;
mod routes;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/pipeline", post(routes::pipeline::handle_pipeline));

    let addr = SocketAddr::from(([0, 0, 0, 0], 6778));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    println!("sandbox-service listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
