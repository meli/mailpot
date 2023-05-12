use mailpot_http::{settings::SETTINGS, *};
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, propagate_header::PropagateHeaderLayer,
    sensitive_headers::SetSensitiveHeadersLayer,
};

use crate::routes;

#[tokio::main]
async fn main() {
    let app = create_app().await;

    let port = SETTINGS.server.port;
    let address = SocketAddr::from(([127, 0, 0, 1], port));

    info!("Server listening on {}", &address);
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .expect("Failed to start server");
}
pub async fn create_app() -> Router {
    let config_path = std::env::args()
        .nth(1)
        .expect("Expected configuration file path as first argument.");
    stderrlog::new()
        .quiet(false)
        .verbosity(15)
        .show_module_names(true)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
    let conf = Arc::new(Configuration::from_file(config_path).unwrap());

    Router::new()
        .with_state(conf.clone())
        .merge(Router::new().nest("/v1", Router::new().merge(routes::list::create_route(conf))))
        .layer(SetSensitiveHeadersLayer::new(std::iter::once(
            header::AUTHORIZATION,
        )))
        // Compress responses
        .layer(CompressionLayer::new())
        // Propagate `X-Request-Id`s from requests to responses
        .layer(PropagateHeaderLayer::new(header::HeaderName::from_static(
            "x-request-id",
        )))
        // CORS configuration. This should probably be more restrictive in
        // production.
        .layer(CorsLayer::permissive())
}
