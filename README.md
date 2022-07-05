# secure-javascript-sandbox

Secure sandbox for JavaScript plugins using Rust and Web Assembly.

## Development Setup

### Wasi

Many rust crates don't work out of the box with wasm32-unknown-unknown because it does not provide functionality like system time and a source of randomness. Many common libraries do conditionally support a target of wasm32-wasi though. To use the `wasm32-wasi` target you may first have to install it by running:

```sh
rustup target add wasm32-wasi
```

### Wasmtime

Wasmtime is a runtime for code that has been compiled to target wasm32-wasi. We use wasmtime as a library, but you can also directly run the .wasm files if you install wasmtime by running:

```sh
curl https://wasmtime.dev/install.sh -sSf | bash
```

### Wasm-NM

The `.wasm` file is a binary format. If you want to read the instructions that were generated, you can extract a text based representation by running:

```sh
cargo install wasm-nm
```

and then running:

```sh
wasm-nm -z target/wasm32-wasi/release/secure_js_sandbox_interpreter_boa.wasm > sandbox.txt
```

## Architecture

This sandbox consists of the following components:

 - _Interpreter_ - This is an interpreter for JavaScript that is compiled to the wasm32-wasi target. My current implementation uses [boa](https://github.com/boa-dev/boa) to actually run the JavaScript, as it is entirely written in Rust, and can be easily compiled to run in wasm32-wasi. The interpreter should prevent the JavaScript accessing the system's disk, network etc. but does not limit the CPU and RAM consumed by the JavaScript.
 - _Host_ - The _Host_ library runs the _Interpreter_ using [wasmtime](https://wasmtime.dev). This enables us to impose limits on CPU usage (fuel) and RAM usage (memory). It also further sandboxes the _Interpreter_ so that any bugs in the _Interpreter_ cannot accidentally permit the JavaScript code to access system resources.
 - _Server_ - The _Server_ is designed to be deployed as a docker image to a service like Google's Cloud Run. It allows a JSON API to be used to call the _Host_. Ideally, this docker image should be deployed with minimal privileges, in order to limit the damage if bugs in the _Interpreter_ and _Host_ were to enable a sandbox escape. It's also a good idea to deploy it in an auto-scaling configuration so that bursts in requests can be easily handled. Having said that, the _Server_ does support multiple concurrent threads, so should be able to handle fairly high request volume on even modest hardware (depending on how much "fuel" and "memory" you allocate to each call).
 - _CLI_ - The _CLI_ is an alternative to the server that lets you directly call the _Host_

### Interpreter

To run the interpreter natively (i.e. without the Host sandbox), you can run:

```sh
cargo run --bin secure_js_sandbox_interpreter_boa
```

To compile the interpreter to wasm32-wasi, you can run:

```sh
cargo build --bin secure_js_sandbox_interpreter_boa --release --target wasm32-wasi
```

This generates the output file: [secure_js_sandbox_interpreter_boa.wasm](target/wasm32-wasi/release/secure_js_sandbox_interpreter_boa.wasm). You can try running this using:

```sh
wasmtime target/wasm32-wasi/release/secure_js_sandbox_interpreter_boa.wasm
```

The interpreter should have a standardized interface, allowing easy experimentation with other JavaScript interpreters in the future:

- [ToyJS](https://github.com/DelSkayn/toyjs) - Relatively new project and probably much more limited than Boa
- [Starlight](https://github.com/Starlight-JS/starlight) - Much less actively maintained than boa
- V8 etc. - probably much harder to compile to web assembly
- JavaScript Core - [JSC.js](https://github.com/mbbill/JSC.js) shows it is possible to compile this to web assembly


### CLI

To run the CLI, you must first compile the _Interpreter_ to `wasm32-wasi`, you can then run:

```sh
cargo run --bin secure_js_sandbox_cli --script "console.log('hello world')"
```

You can also run a benchmark comparing the `secure_js_sandbox_cli` against an insecure attempt at using node.js to create a sandbox by running `zsh tests/benchmark.zsh`

### Server

To run the Server, you must first compile the _Interpreter_ to `wasm32-wasi`, you can then run:

```sh
cargo run --bin secure_js_sandbox_server
```

#### POST `/execute`

Example:

```sh
  time curl -X POST http://localhost:3000/execute \
    -H 'Content-Type: application/json' \
    -d '{"sandbox_id": "x", "init_script": "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }", "script": "fib(13)"}';
```

Request:

```typescript
interface RequestBody {
  /**
   * The sandbox ID should be a unique ID per sandbox you want to use.
   * No two different users should have the same sandbox id. You can
   * pass `null` to disable sandbox reuse between requests.
   */
  sandbox_id: string | null;
  /**
   * Script to run before "script" in order to initialize new sandboxes.
   * This is especially useful when combined with a "sandbox_id" as it
   * lets you run some setup code once, and re-use the result across
   * many calls to the script. Note that sandbox re-use is only ever
   * on a best-effort basis, so your code should never rely on sandbox
   * reuse to function correctly.
   */
  init_script: string;
  /**
   * The JavaScript code to run on each request.
   */
  script: string;
}
```

Response:

```typescript
/**
 * OK indicates that the JavaScript
 * code was successfully evaluated.
 * The "result" field contains the
 * value of the final expression that
 * was evaluated (providing it can be)
 * serialized to JSON.
 * 
 * Status Code = 200
 */
interface ResponseOk {
  status: "OK";
  result: unknown;
  stdout: string;
  stderr: string;
}

/**
 * RUNTIME_ERROR indicates that a runtime error
 * was thrown by JavaScript while attempting to
 * process the request.
 * 
 * Stack traces are currently not supported, but
 * may be added in a future release.
 * 
 * Status Code = 400
 */
interface ResponseRuntimeError {
  status: "RUNTIME_ERROR";
  stage: "INIT" | "SCRIPT";
  message: string;
  stdout: string;
  stderr: string;
}

/**
 * OUT_OF_FUEL indicates that too much CPU time was
 * consumed while attempting to process the request.
 * 
 * Status Code = 400
 */
interface ResponseOutOfFuel {
  status: "OUT_OF_FUEL";
  stage: "INIT" | "SCRIPT";
  message: string;
  stdout: string;
  stderr: string;
}

/**
 * OUT_OF_MEMORY indicates that too much memory was
 * consumed by this JavaScript sandbox.
 * 
 * Status Code = 400
 */
interface ResponseOutOfMemory {
  status: "OUT_OF_MEMORY";
  stage: "INIT" | "SCRIPT";
  message: string;
  stdout: string;
  stderr: string;
}

/**
 * INVALID_REQUEST indicates that the request body
 * did not match the expected schema.
 * 
 * Status Code = 400
 */
interface ResponseInvalidRequest {
  status: "INVALID_REQUEST";
  message: string;
}

/**
 * INTERNAL_SERVER_ERROR indicates that some unknown
 * error occurred while attempting to process the
 * request. This normally indicates a bug in
 * secure-js-sandbox.
 * 
 * Status Code = 500
 */
interface ResponseInternalServerError {
  status: "INTERNAL_SERVER_ERROR";
  stage?: "INIT" | "SCRIPT";
  message: string;
}

type ResponseBody =
  | ResponseOk
  | ResponseRuntimeError
  | ResponseOutOfFuel
  | ResponseOutOfMemory
  | ResponseInvalidRequest
  | ResponseInternalServerError
```