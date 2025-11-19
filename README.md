# secure-javascript-sandbox

Secure sandbox for JavaScript plugins using Rust and Web Assembly.

## Architecture

This sandbox consists of the following components:

 - `wasm-sandbox` is a JavaScript component compiled to WASM using [ComponentizeJS](https://github.com/bytecodealliance/ComponentizeJS), which embeds the Mozilla SpiderMonkey JavaScript engine inside the WebAssembly component.
 - `crates/sandbox` provides the code to load this WebAssembly component and send it a JavaScript function to run and some JSON serialized parameters. It limits the "fuel" (approximately CPU cycles) and memory used by the WebAssembly component, and exposes only a few select APIs to the WebAssembly component to ensure it only has access to call the URLs you choose.
 - `crates/axum_handler` provides helpers for defining a web endpoint for the axum Rust web server framework.
 - `crates/server` provides a ready to use server that can be configured via environment variables.

## Usage

### Run the server using docker

```sh
docker run --rm -p "3000:3000" forbeslindesay/secure-js-sandbox
```

### Config

The server can be configured using these environment variables (defaults shown here):

```toml
# Host to listen on
HOST="0.0.0.0"
# Port number to listen on
PORT="3000"

# Set this to true to allow passing the sandbox config
# options as JSON in the request body instead of setting
# them via environment variables.
# This would be much less secure.
SANDBOX_ALLOW_CONFIG_IN_REQUEST="FALSE"

# How many CPU cycles to allow per request. This corresponds
# to about 100ms on my 2024 MacBook Pro
SANDBOX_CPU_FUEL="440_000_000"
# How much memory (in bytes) to allow each sandboxed function
# to use. This includes the memory for the Spidermonkey VM
# itself. Defaults to 128MB.
SANDBOX_MAX_MEMORY_BYTES="134_217_728"
# Set a limit on the number of "tables elements" within the WASM VM
SANDBOX_MAX_TABLE_ELEMENTS="100_000"
# Set a limit on the number of "instances" within the WASM VM
SANDBOX_MAX_INSTANCES="10_000"
# Set a limit on the number of "tables" within the WASM VM
SANDBOX_MAX_TABLES="10_000"
# Set a limit on the number of "memories" within the WASM VM
SANDBOX_MAX_MEMORIES="10_000"
# Enable this to throw a WASM error when running out of memory,
# instead of the default JavaScript out of memory error.
SANDBOX_TRAP_ON_GROW_FAILURE="FALSE"
# The maximum number of bytes of stdout (i.e. console.log) to
# record. If stdout exceeds this limit, andy further data will
# just be dropped.
SANDBOX_STDOUT_MAX_BYTES="10_485_760"
# The maximum number of bytes of stderr (i.e. console.error) to
# record. If stderr exceeds this limit, andy further data will
# just be dropped.
SANDBOX_STDERR_MAX_BYTES="10_485_760"
# Whether to allow outbound requests via the `fetch` function.
SANDBOX_HTTP_MODE="DISABLED"
```

There are 4 possible values for `SANDBOX_HTTP_MODE`

* `ALLOW_ALL` - allows all outbound requests without any restrictions.
* `ALLOW_GLOBAL_IP_ONLY` - allows outbound requests only if the target is an IP address that's considered "Global".
* `ALLOW_LIST_HOSTS:{hosts,}*` - allows outbound requests only to the specified list of host names. e.g. `ALLOW_LIST_HOSTS:example.com,example.org` would allow fetch requests to `example.com` and `example.org` but not to `example.net`.
* `DISABLED` - blocks all outbound requests.

### API

#### POST `/evaluate`

Example:

```sh
  time curl -X POST http://localhost:3000/evaluate \
    -H 'Content-Type: application/json' \
    -d '{"script": "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }", "args": [13]}';
```

Request:

```typescript
interface EvaluateRequest {
  /**
   * A function expression to be evaluated. The function can be async, allowing for the use
   * of `fetch` and things like timers.
   */
  script: string;
  /**
   * A list of arguments to pass in to the function defined by `script`.
   */
  args: unknown[];
}
```

Response:

```typescript
interface EvaluateResponse {
  /**
   * True if the function ran without any errors.
   */
  success: boolean;
  /**
   * The value returned by the function if success is true,
   * otherwise this will be an object in the form {error: string}
   */
  result: unknown;
  stdout: string;
  stderr: string;
  fuel_consumed: number;
  fuel_remaining: number;
  max_requested_memory_bytes: number;
  max_requested_table_elements: number;
  outbound_requests: {
    outcome: "ALLOWED" | "BLOCKED";
    uri: string;
    socket_addr: string | null;
  }[]
}
```

## Development Setup

1. Build the wasm-sandbox by running `cd wasm-sandbox && npm install && npm build`
2. Run the server using `cargo run --bin secure_js_sandbox_server`
3. Run tests via `zsh tests/some-file.zsh`

You can build the docker image by running:

```sh
docker build -t forbeslindesay/secure-js-sandbox .
```

You can run the docker image by running:

```sh
docker run --rm -it -p "3000:3000" forbeslindesay/secure-js-sandbox
```

To publish the image:

```sh
docker login

docker buildx create \
  --name container \
  --driver=docker-container

docker buildx build \
  --tag forbeslindesay/secure-js-sandbox:latest \
  --platform linux/arm/v7,linux/arm64/v8,linux/amd64 \
  --builder container \
  --push .
```