mod auth;
mod bootstrap;
mod config;
mod db;
mod error;
mod llm;
mod models;
mod routes;
mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use tokio::sync::{Mutex, Semaphore};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    config::Config,
    llm::LlmRuntime,
    state::{AppState, LoginLimiter},
};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "jarvis_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env().unwrap_or_else(|e| {
        eprintln!("config error: {e}");
        std::process::exit(1);
    });
    let config = Arc::new(config);

    let pool = db::connect(&config.database_url)
        .await
        .unwrap_or_else(|e| {
            eprintln!("db error: {e}");
            std::process::exit(1);
        });
    db::migrate(&pool).await.unwrap_or_else(|e| {
        eprintln!("migrate error: {e}");
        std::process::exit(1);
    });
    bootstrap::ensure_admin(&pool, &config)
        .await
        .unwrap_or_else(|e| {
            eprintln!("bootstrap error: {e}");
            std::process::exit(1);
        });

    let llm = LlmRuntime::from_config(&config).await.unwrap_or_else(|e| {
        eprintln!("llm init error: {e}");
        std::process::exit(1);
    });

    let state = AppState {
        pool,
        llm: Arc::new(llm),
        llm_inflight: Arc::new(Semaphore::new(config.llm_max_inflight.max(1))),
        login_limiter: Arc::new(Mutex::new(LoginLimiter::new())),
        config: config.clone(),
    };

    let cors = build_cors(&config.cors_origins);

    let app = routes::router()
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("jarvis-api listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind failed");
    axum::serve(listener, app).await.expect("server error");
}

fn build_cors(origins: &[String]) -> CorsLayer {
    let mut layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]);

    if origins.iter().any(|o| o == "*") {
        layer = layer.allow_origin(AllowOrigin::any());
    } else {
        let parsed: Vec<HeaderValue> = origins
            .iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect();
        layer = layer.allow_origin(AllowOrigin::list(parsed));
    }
    layer
}
