cargo run --bin secure_js_sandbox_cli -- --script "throw new Error('something went wrong')"
echo "Exit code: $?"

cargo run --bin secure_js_sandbox_cli -- --script "function fib(n) { /* console.log('fib(' + n + ')'); */ return n <= 1 ? 1 : fib(n-1) + fib(n-2); }; fib(20)"
echo "Exit code: $?"

cargo run --bin secure_js_sandbox_cli -- --script "const result = []; while (true) result.push(new ArrayBuffer(1024));" --memory-limit-bytes 10000000 --fuel 999999999999
echo "Exit code: $?"

cargo run --bin secure_js_sandbox_cli -- --script "const result = []; while (true) result.push(new ArrayBuffer(1024));" --memory-limit-bytes 5000000 --fuel 999999999999
echo "Exit code: $?"
