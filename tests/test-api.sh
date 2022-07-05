for V in {1..10}; do
  time curl -X POST http://localhost:3000/execute \
    -H 'Content-Type: application/json' \
    -d '{"sandbox_id": "x", "init_script": "let i = 0;function fn() {return ++i}", "script": "fn()"}';
done

for V in {1..10}; do
  time curl -X POST http://localhost:3000/execute \
    -H 'Content-Type: application/json' \
    -d '{"sandbox_id": "x", "init_script": "function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }", "script": "fib(13)"}';
done


for i in {1..2000}; do
  time curl -X POST http://localhost:3000/execute \
    -H 'Content-Type: application/json' \
    -d "{\"sandbox_id\": \"$i\", \"init_script\": \"function fib(n) { return n <= 1 ? 1 : fib(n-1) + fib(n-2); }var result = fib(18)\", \"script\": \"result\"}";
done