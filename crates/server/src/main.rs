#![deny(warnings)]

use axum::{
    Router,
    routing::{get, post},
};
use secure_js_sandbox_axum_handler::{
    AllowRequestToConfigureSandbox, SandboxServerConfig, create_evaluate_handler, get_env,
};
use std::error::Error;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // build our application with a route
    let mut app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root));
    if std::env::var("SANDBOX_ALLOW_CONFIG_IN_REQUEST")
        .ok()
        .is_some_and(|s| s == "TRUE")
    {
        app = app.route(
            "/evaluate",
            post(create_evaluate_handler(AllowRequestToConfigureSandbox).await?),
        );
    } else {
        app = app.route(
            "/evaluate",
            post(create_evaluate_handler(SandboxServerConfig::from_env()?).await?),
        );
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
