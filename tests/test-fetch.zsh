time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"script": "async function fn(url) { const res = await fetch(url); console.log(res.status); return await res.text() }", "args": ["https://example.com"]}';

time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"script": "async function fn(url) { const res = await fetch(url); console.log(res.status, res.url); return await res.text() }", "args": ["https://dns.forbeslindesay.co.uk"]}';