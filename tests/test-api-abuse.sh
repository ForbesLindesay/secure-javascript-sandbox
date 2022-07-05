for V in {1..1000}; do
  time curl -X POST http://localhost:3000/execute \
    -H 'Content-Type: application/json' \
    -d '{"sandbox_id": "abuse", "init_script": "function foo(a, b) {while (true);}", "script": "foo(1, `hello world`)"}';
done