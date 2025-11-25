#![deny(warnings)]

use axum::{
    Router,
    routing::{get, post},
};
use secure_js_sandbox_axum_handler::{
    AllowRequestToConfigureSandbox, SandboxServerConfig, create_evaluate_function_handler,
    create_evaluate_module_handler, create_strip_types_handler, get_env,
};

mod signal;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
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
    if get_env("SANDBOX_USE_MODULE_SYNTAX")?.unwrap_or(false) {
        if get_env("SANDBOX_ALLOW_CONFIG_IN_REQUEST")?.unwrap_or(false) {
            app = app.route(
                "/evaluate",
                post(create_evaluate_module_handler(AllowRequestToConfigureSandbox).await?),
            );
        } else {
            app = app.route(
                "/evaluate",
                post(create_evaluate_module_handler(SandboxServerConfig::from_env()?).await?),
            );
        }
    } else {
        if get_env("SANDBOX_ALLOW_CONFIG_IN_REQUEST")?.unwrap_or(false) {
            app = app.route(
                "/evaluate",
                post(create_evaluate_function_handler(AllowRequestToConfigureSandbox).await?),
            );
        } else {
            app = app.route(
                "/evaluate",
                post(create_evaluate_function_handler(SandboxServerConfig::from_env()?).await?),
            );
        }
    }

    if get_env("SANDBOX_ENABLE_STRIP_TYPES_ENDPOINT")?.unwrap_or(false) {
        app = app.route("/strip_types", post(create_strip_types_handler()));
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
