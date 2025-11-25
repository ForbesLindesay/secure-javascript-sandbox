# for V in {1..1000}; do
#   time curl -X POST http://localhost:3000/evaluate \
#     -H 'Content-Type: application/json' \
#     -d '{"script": "function foo(a, b) {while (true);}", "args": []}';
# done

echo "While loop test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"script": "function foo(a, b) {while (true);}", "args": []}';

echo "Acceptable recursion test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"script": "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }", "args": [20]}';

echo "Excessive recursion test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"script": "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }", "args": [30]}';

echo "Excessive memory test:"
time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"script": "function fn() { let result = new Uint8Array(1024 * 1024 * 128); return result.byteLength }", "args": []}';
