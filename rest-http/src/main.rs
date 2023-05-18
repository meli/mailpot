use mailpot_http::{settings::SETTINGS, *};

use crate::create_app;

#[tokio::main]
async fn main() {
    let config_path = std::env::args()
        .nth(1)
        .expect("Expected configuration file path as first argument.");
    #[cfg(test)]
    let verbosity = log::LevelFilter::Trace;
    #[cfg(not(test))]
    let verbosity = log::LevelFilter::Info;
    stderrlog::new()
        .quiet(false)
        .verbosity(verbosity)
        .show_module_names(true)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();
    let conf = Arc::new(Configuration::from_file(config_path).unwrap());
    let app = create_app(conf);

    let port = SETTINGS.server.port;
    let address = SocketAddr::from(([127, 0, 0, 1], port));

    info!("Server listening on {}", &address);
    axum::Server::bind(&address)
        .serve(app.into_make_service())
        .await
        .expect("Failed to start server");
}
