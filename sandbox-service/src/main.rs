use axum::{routing::post, Router};
use std::net::SocketAddr;

mod models;
mod routes;
mod ast; 


#[tokio::main]
async fn main() {
    let app = Router::new().route("/pipeline", post(routes::pipeline::handle_pipeline));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    println!("sandbox-service listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
