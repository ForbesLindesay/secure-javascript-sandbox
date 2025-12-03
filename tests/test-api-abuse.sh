# for V in {1..1000}; do
#   time curl -X POST http://localhost:3000/evaluate \
#     -H 'Content-Type: application/json' \
#     -d '{"code": "function foo(a, b) {while (true);}", "parameters": []}';
# done

echo "While loop test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"code": "function foo(a, b) {while (true);}", "parameters": []}';

echo "Acceptable recursion test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"code": "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }", "parameters": [20]}';

echo "Excessive recursion test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"code": "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }", "parameters": [30]}';

echo "Excessive memory test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"code": "function fn() { let result = new Uint8Array(1024 * 1024 * 128); return result.byteLength }", "parameters": []}';
