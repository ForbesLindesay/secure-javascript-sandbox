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
cargo run --bin secure_js_sandbox_cli
```

### Server

To run the Server, you must first compile the _Interpreter_ to `wasm32-wasi`, you can then run:

```sh
cargo run --bin secure_js_sandbox_server
```
