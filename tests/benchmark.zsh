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

printsecure "Evaluating fib(13) 1000 times using wasm sandbox (single threaded)"
time node insecure-nodejs-sandbox --endpoint "http://localhost:3000/evaluate" --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }" --args "[13]" --repeat 1000
echo ""

printsecure "Evaluating fib(13) 1000 times using wasm sandbox (16 threads)"
time node insecure-nodejs-sandbox --endpoint "http://localhost:3000/evaluate" --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }" --args "[13]" --repeat 1000 --threads 16
echo ""

printinsecure "Evaluating fib(13) 1000 times using node.js (single threaded)"
time node insecure-nodejs-sandbox --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }" --args "[13]" --repeat 1000
echo ""

printinsecure "Evaluating fib(13) 1000 times using node.js (16 threads)"
time node insecure-nodejs-sandbox --quiet --script "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }" --args "[13]" --repeat 1000 --threads 16
echo ""
