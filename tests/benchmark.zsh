#!/bin/zsh

set -euo pipefail

# '\nreal\t%E\nuser\t%U\nsys\t%S'
export TIMEFMT=$'%E real (%U user, %S sys)'

function printsecure {
  print -P "%F{cyan}$1%f - %F{green}This is a secure sandbox%f"
}
function printsecure2 {
  print -P "%F{cyan}$1%f - %F{green}This is a secure sandbox (if all scripts are by the same author)%f"
}

function printinsecure {
  print -P "%F{cyan}$1%f - %F{red}This is not a secure sandbox%f"
}

print -P "%F{cyan}Building production release of secure_js_sandbox_cli%f"
cargo install --path crates/cli
echo ""

printinsecure "Evaluating fib(13) 100 times using node.js (single threaded)"
time node insecure-nodejs-sandbox --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }; fib(13)" --repeat 100
echo ""

printinsecure "Evaluating fib(13) 100 times using node.js (16 threads)"
time node insecure-nodejs-sandbox --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }; fib(13)" --repeat 100 --threads 16
echo ""

printsecure "Evaluating fib(13) 1000 times using wasm sandbox (single threaded)"
time secure_js_sandbox_cli --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }; fib(13)" --repeat 1000
echo ""

printsecure "Evaluating fib(13) 1000 times using wasm sandbox (16 threads)"
time secure_js_sandbox_cli --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }; fib(13)" --repeat 1000 --threads 16
echo ""

printsecure2 "Evaluating fib(13) 1000 times reusing a single wasm sandbox (single threaded)"
time secure_js_sandbox_cli --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }; fib(13)" --repeat 1000 --reuse
echo ""

printsecure2 "Evaluating fib(13) 1000 times reusing a single wasm sandbox (16 threads)"
time secure_js_sandbox_cli --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }; fib(13)" --repeat 1000 --threads 16 --reuse
echo ""
