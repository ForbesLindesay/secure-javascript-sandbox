time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"code": "async function fn(url) { const res = await fetch(url); console.log(res.status); return await res.text() }", "parameters": ["https://example.com"]}';

time curl -X POST http://localhost:3000/evaluate \
  -H 'Content-Type: application/json' \
  -d '{"code": "async function fn(url) { const res = await fetch(url); console.log(res.status, res.url); return await res.text() }", "parameters": ["https://dns.forbeslindesay.co.uk"]}';