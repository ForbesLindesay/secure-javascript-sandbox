#![deny(warnings)]

use axum::{Router, routing::get};
use secure_js_sandbox_axum_handler::{
    AllowRequestToConfigureSandbox, SandboxServerConfig, TsUtilsHandler, create_evaluate_handler,
    create_strip_types_handler, create_validate_module_handler, get_env,
};

mod signal;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install default TLS provider");

    let main = start_server();
    let signal_listener = signal::listen_signal();
    tokio::select! {
        res = main => res,
        res = signal_listener => res,
    }
}

pub async fn start_server() -> anyhow::Result<()> {
    // build our application with a route
    let mut app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root));
    if get_env("SANDBOX_ALLOW_CONFIG_IN_REQUEST")?.unwrap_or(false) {
        app = app.route(
            "/evaluate",
            create_evaluate_handler(AllowRequestToConfigureSandbox::from_env()?).await?,
        );
    } else {
        app = app.route(
            "/evaluate",
            create_evaluate_handler(SandboxServerConfig::from_env()?).await?,
        );
    }

    let enable_strip_types_endpoint =
        get_env("SANDBOX_ENABLE_STRIP_TYPES_ENDPOINT")?.unwrap_or(false);
    let enable_validate_module_endpoint =
        get_env("SANDBOX_ENABLE_VALIDATE_MODULE_ENDPOINT")?.unwrap_or(false);
    if enable_strip_types_endpoint || enable_validate_module_endpoint {
        let handler = TsUtilsHandler::from_env()?;
        if enable_strip_types_endpoint {
            app = app.route("/strip_types", create_strip_types_handler(handler.clone()));
        }
        if enable_validate_module_endpoint {
            app = app.route("/validate_module", create_validate_module_handler(handler));
        }
    }

    // run our app with hyper, listening globally on port 3000
    let host: String = get_env("HOST")?.unwrap_or("0.0.0.0".to_string());
    let port: u16 = get_env("PORT")?.unwrap_or(3000);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    println!("Listening on http://{}", &addr);

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Usage: POST /evaluate with JSON body"
}
