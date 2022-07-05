#![deny(warnings)]

use clap::{Parser};
use hyper::{service::{service_fn, make_service_fn}, Request, Body, Response, Method, StatusCode, Server, body::to_bytes, header::HeaderValue};
use secure_js_sandbox_host::{JsSandboxContext, MemoryLimits, JsRunOutput};
use serde_json::{json, to_string, Value as JsonValue, from_str};
use tokio::task::spawn_blocking;
// use secure_js_sandbox_host as host;
use std::{error::Error, convert::Infallible};
use serde::Deserialize;
use lazy_static::lazy_static;

lazy_static! {
    static ref SANDBOX_STORE: sandbox_store::SandboxStore = sandbox_store::SandboxStore::new();
}

#[derive(Parser, Clone, Copy, Debug)]
struct Config {
    #[clap(long, env, value_parser, default_value_t = 3000)]
    port: u16,

    /// Limit to 128MB of data in the sandbox cache
    #[clap(long, env, value_parser, default_value_t = 128 * 1024 * 1024)]
    memory_limit_bytes_sandbox_cache: usize,

    /// Limit to 50MB per sandbox by default
    #[clap(long, env, value_parser, default_value_t = 50 * 1024 * 1024)]
    memory_limit_bytes_per_sandbox: usize,

    /// I think this limits number of methods/exports in table, defaults to 10,000
    #[clap(long, env, value_parser, default_value_t = 10_000)]
    max_table_elements_per_sandbox: u32,

    /// The "fuel" for CPU operations. 440 million is approximately 100ms on
    /// my MacBook Pro.
    #[clap(long, env, value_parser, default_value_t = 440_000_000)]
    fuel_per_init: u64,

    /// The "fuel" for CPU operations. 440 million is approximately 100ms on
    /// my MacBook Pro.
    #[clap(long, env, value_parser, default_value_t = 440_000_000)]
    fuel_per_call: u64,
}

#[derive(Deserialize, Debug)]
struct RequestBody {
    sandbox_id: Option<String>,
    init_script: Option<String>,
    script: String
}

enum ResponseStage {
    Init,
    Script,
}
impl ResponseStage {
    fn to_string(&self) -> String {
        match self {
            ResponseStage::Init => "INIT".to_string(),
            ResponseStage::Script => "SCRIPT".to_string(),
        }
    }
}

enum ResponseBody {
    ParseBodyError(String),
    InternalError(ResponseStage, String),
    Ok {
        result: Option<JsonValue>,
        stdout: String,
        stderr: String,
    },
    RuntimeError {
        stage: ResponseStage,
        message: String,
        stdout: String,
        stderr: String,
    },
    OutOfFuel {
        stage: ResponseStage,
        stdout: String,
        stderr: String,
    },
    OutOfMemory {
        stage: ResponseStage,
        stdout: String,
        stderr: String,
    },
}
impl ResponseBody {
    fn status_code(&self) -> StatusCode {
        match self {
            ResponseBody::ParseBodyError(_) => StatusCode::BAD_REQUEST,
            ResponseBody::InternalError(_, _) => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseBody::Ok{result:_,stdout:_,stderr:_} => StatusCode::OK,
            ResponseBody::RuntimeError { stage:_, message: _, stdout: _, stderr: _ } => StatusCode::BAD_REQUEST,
            ResponseBody::OutOfFuel { stage:_, stdout: _, stderr: _ } => StatusCode::BAD_REQUEST,
            ResponseBody::OutOfMemory { stage:_, stdout: _, stderr: _ } => StatusCode::BAD_REQUEST,
        }
    }
    fn response_json(self) -> JsonValue {
        match self {
            ResponseBody::ParseBodyError(message) => json!({
                "status": "INVALID_REQUEST",
                "message": message
            }),
            ResponseBody::InternalError(stage, message) => json!({
                "status": "INTERNAL_SERVER_ERROR",
                "stage": stage.to_string(),
                "message": message
            }),
            ResponseBody::Ok{result,stdout,stderr} => json!({
                "status": "OK",
                "result": result,
                "stdout": stdout,
                "stderr": stderr,
            }),
            ResponseBody::RuntimeError { stage, message, stdout, stderr } => json!({
                "status": "RUNTIME_ERROR",
                "stage": stage.to_string(),
                "message": message,
                "stdout": stdout,
                "stderr": stderr,
            }),
            ResponseBody::OutOfFuel { stage, stdout, stderr } => json!({
                "status": "OUT_OF_FUEL",
                "message": "Ran out of CPU time while evaluating the script",
                "stage": stage.to_string(),
                "stdout": stdout,
                "stderr": stderr,
            }),
            ResponseBody::OutOfMemory { stage, stdout, stderr } => json!({
                "status": "OUT_OF_MEMORY",
                "message": "Ran out of memory while evaluating the script",
                "stage": stage.to_string(),
                "stdout": stdout,
                "stderr": stderr,
            }),
        }
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse();
    println!("{config:?}");

    SANDBOX_STORE.set_memory_limit(Some(config.memory_limit_bytes_sandbox_cache));
    // For every connection, we must make a `Service` to handle all
    // incoming HTTP requests on said connection.
    let make_svc = make_service_fn(|_conn| {
        // This is the `Service` that will handle the connection.
        // `service_fn` is a helper to convert a function that
        // returns a Response into a `Service`.
        async move { Ok::<_, Infallible>(service_fn(move |req| handle_request(req, config))) }
    });

    let addr = ([0, 0, 0, 0], config.port).into();

    let server = Server::bind(&addr).serve(make_svc);

    // write_log(
    //     "INFO",
    //     "LISTENING",
    //     &format!("Listening on http://{}", addr),
    // );

    server.await?;

    Ok(())
}

async fn handle_request(req: Request<Body>, config: Config) -> Result<Response<Body>, Infallible> {
    Ok(handle_request_internal(req, config).await)
}

async fn handle_request_internal(
    req: Request<Body>,
    config: Config,
) -> Response<Body> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => json_response(StatusCode::OK, json!({
            "memory_limit_bytes_sandbox_cache": config.memory_limit_bytes_sandbox_cache,
            "memory_limit_bytes_per_sandbox": config.memory_limit_bytes_per_sandbox,
            "max_table_elements_per_sandbox": config.max_table_elements_per_sandbox,
            "fuel_per_init": config.fuel_per_init,
            "fuel_per_call": config.fuel_per_call,
            "memory_consumed": SANDBOX_STORE.memory_consumed()
        })),

        (&Method::POST, "/execute") => {
            let response_body = handle_js_request(req, config).await;
            json_response(
                response_body.status_code(),
                response_body.response_json(),
            )
        }

        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::new(Body::from("Page not found"));
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            not_found
        }
    }
}


async fn handle_js_request(
    req: Request<Body>,
    config: Config,
) -> ResponseBody {
    let body_bytes = match to_bytes(req.into_body())
        .await {
            Ok(body_bytes) => body_bytes,
            Err(_) => return ResponseBody::ParseBodyError("Missing request body".to_string())
        };
    let body_str = match String::from_utf8(body_bytes.to_vec()) {
        Ok(body_str) => body_str,
        Err(_) => return ResponseBody::ParseBodyError("Body is not a valid utf8 string".to_string())
    };
    let request_body: RequestBody = match from_str(&body_str) {
        Ok(request_body) => request_body,
        Err(e) => return ResponseBody::ParseBodyError(format!("Body is not a valid request: {e}"))
    };
    match spawn_blocking(move || {
        execute_js_request(request_body, config)
    }).await {
        Ok(response) => response,
        // TODO: log the error
        Err(_) => ResponseBody::InternalError(ResponseStage::Init, "Internal server error".to_string())
    }
}
fn execute_js_request(req: RequestBody, config: Config) -> ResponseBody {
    let ctx = match &req.sandbox_id {
        Some(id) => SANDBOX_STORE.get(id, &req.init_script),
        None => None
    };
    let mut ctx = match ctx {
        Some(ctx) => ctx,
        None => {
            let mut ctx = JsSandboxContext::new(&MemoryLimits {
                max_bytes: config.memory_limit_bytes_per_sandbox,
                max_table_elements: config.max_table_elements_per_sandbox,
            });
            ctx.add_fuel(config.fuel_per_init);
            match &req.init_script {
                Some(init_script) => {
                    let init_result = match ctx.run(&init_script) {
                        Ok(init_result) => init_result,
                        Err(_e) => {
                            // TODO: log this error
                            return ResponseBody::InternalError(ResponseStage::Init, "Internal error while evaluating init_script.".to_string());
                        }
                    };
                    match init_result {
                        JsRunOutput::Ok { ctx, result:_, stdout: _, stderr: _ } => {
                            ctx
                        },
                        JsRunOutput::RuntimeError { ctx: _, message, stdout, stderr } => {
                            return ResponseBody::RuntimeError { stage: ResponseStage::Init, message, stdout, stderr }
                        },
                        JsRunOutput::OutOfFuel { stdout, stderr } => {
                            return ResponseBody::OutOfFuel { stage: ResponseStage::Init, stdout, stderr }
                        },
                        JsRunOutput::OutOfMemory { stdout, stderr } => {
                            return ResponseBody::OutOfMemory { stage: ResponseStage::Init, stdout, stderr }
                        }
                    }
                },
                None => ctx
            }
        }
    };
    let fuel_remaining = ctx.fuel_remaining();
    if fuel_remaining < config.fuel_per_call {
        ctx.add_fuel(config.fuel_per_call - fuel_remaining);
    }
    let script_result = match ctx.run(&req.script) {
        Ok(script_result) => script_result,
        Err(_e) => {
            // TODO: log this error
            return ResponseBody::InternalError(ResponseStage::Script, "Internal error while evaluating script.".to_string());
        }
    };
    match script_result {
        JsRunOutput::Ok { ctx, result, stdout, stderr } => {
            if let Some(id) = &req.sandbox_id {
                SANDBOX_STORE.set(id, req.init_script, ctx);
            }
            return ResponseBody::Ok { result, stdout, stderr }
        },
        JsRunOutput::RuntimeError { ctx, message, stdout, stderr } => {
            if let Some(id) = &req.sandbox_id {
                SANDBOX_STORE.set(id, req.init_script, ctx);
            }
            return ResponseBody::RuntimeError { stage: ResponseStage::Script, message, stdout, stderr }
        },
        JsRunOutput::OutOfFuel { stdout, stderr } => {
            return ResponseBody::OutOfFuel { stage: ResponseStage::Script, stdout, stderr }
        },
        JsRunOutput::OutOfMemory { stdout, stderr } => {
            return ResponseBody::OutOfMemory { stage: ResponseStage::Script, stdout, stderr }
        }
    }

}


fn json_response(status: StatusCode, data: JsonValue) -> Response<Body> {
    let (new_status, json_string) = match to_string(&data) {
        Ok(json_string) => (status, json_string + "\n"),
        Err(_) => {
            // TODO: log the error here as "ERROR" severity
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from(
                    "{\"code\": \"INTERNAL_ERROR\", \"message\": \"Internal server error\"}\n",
                ),
            )
        }
    };
    let mut response = Response::new(Body::from(json_string));
    response
        .headers_mut()
        .append("Content-Type", HeaderValue::from_static("application/json"));
    *response.status_mut() = new_status;
    response
}

// TODO: make it easy to have limits on the capacity of the SandboxStore
// https://gist.github.com/matey-jack/3e19b6370c6f7036a9119b79a82098ca may be a useful starting point
mod sandbox_store {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use secure_js_sandbox_host::JsSandboxContext;
    use index_list::{IndexList, Index};

    struct ReusableSandboxContext {
        id: String,
        init_script: Option<String>,
        ctx: JsSandboxContext,
        memory_consumed: usize,
    }

    fn init_scripts_equal(a: &Option<String>, b: &Option<String>) -> bool {
        match a {
            None => b.is_none(),
            Some(a) => match b {
                Some(b) => a == b,
                None => false,
            }
        }
    }

    struct SandboxStoreCore {
        memory_limit: Option<usize>,
        memory_consumed: usize,
        map: HashMap<String, Index>,
        list: IndexList<ReusableSandboxContext>
    }

    impl SandboxStoreCore {
        pub fn new(memory_limit: Option<usize>) -> SandboxStoreCore {
            SandboxStoreCore{
                memory_limit,
                memory_consumed: 0,
                map: HashMap::new(),
                list: IndexList::new(),
            }
        }
        pub fn memory_consumed(&self) -> usize {
            self.memory_consumed
        }
        pub fn get(&mut self, id: &str, init_script: &Option<String>) -> Option<JsSandboxContext> {
            let index = match self.map.remove(id) {
                Some(index) => index,
                None => return None,
            };

            let result = match self.list.remove(index) {
                Some(result) => result,
                None => return None,
            };
            self.memory_consumed = self.memory_consumed - result.memory_consumed;
            if init_scripts_equal(&result.init_script, init_script) {
                Some(result.ctx)
            } else {
                None
            }
        }
        pub fn set(&mut self, id: &str, init_script: Option<String>, mut ctx: JsSandboxContext) -> () {
            let memory_consumed = ctx.memory_consumed();
            self.memory_consumed = self.memory_consumed + memory_consumed;
            let index = self.list.insert_last(ReusableSandboxContext {
                id: id.to_string(),
                init_script,
                ctx,
                memory_consumed,
            });
            self.map.insert(id.to_string(), index);
            self.shrink_to_fit();
        }
        pub fn set_memory_limit(&mut self, memory_limit: Option<usize>) {
            self.memory_limit = memory_limit;
            self.shrink_to_fit();
        }
        fn shrink_to_fit(&mut self) {
            if let Some(memory_limit) = self.memory_limit {
                while self.memory_consumed > memory_limit {
                    if let Some(record) = self.list.remove_first() {
                        self.memory_consumed = self.memory_consumed - record.memory_consumed;
                        self.map.remove(&record.id);
                    } else {
                        panic!("Over memory limit but there are no contexts to remove")
                    }
                }
            }
        }
    }

    pub struct SandboxStore(Arc<Mutex<SandboxStoreCore>>);
    unsafe impl Send for SandboxStore {}
    unsafe impl Sync for SandboxStore {}
    impl SandboxStore {
        pub fn new() -> SandboxStore {
            SandboxStore(Arc::new(Mutex::new(SandboxStoreCore::new(None))))
        }
        pub fn memory_consumed(&self) -> usize {
            self.0.lock().unwrap().memory_consumed()
        }
        pub fn get(&self, id: &str, init_script: &Option<String>) -> Option<JsSandboxContext> {
            self.0.lock().unwrap().get(id, init_script)
        }
        pub fn set(&self, id: &str, init_script: Option<String>, ctx: JsSandboxContext) -> () {
            self.0.lock().unwrap().set(id, init_script, ctx)
        }
        pub fn set_memory_limit(&self, memory_limit: Option<usize>) -> () {
            self.0.lock().unwrap().set_memory_limit(memory_limit)
        }
    }
}