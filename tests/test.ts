// node --experimental-strip-types tests/test.ts

import { createServer } from 'node:http';
import { spawn } from 'node:child_process';

const killProc = spawn('zsh', ['-c', 'lsof -t -i tcp:3000 | xargs kill -9'], { stdio: 'inherit' });
await new Promise((resolve) => killProc.on('exit', resolve));

const server = createServer((req, res) => {
  console.log(req.method, req.url);
  if (req.url === '/fib.js') {
    res.writeHead(200, { 'Content-Type': 'text/javascript' });
    res.end(`
      export function fib(n) {
        if (n <= 1) return n;
        return fib(n - 1) + fib(n - 2);
      }
    `.trim());
    return;
  }
  res.writeHead(404);
  res.end("Not Found");
});
server.listen(3001);

const buildProc = spawn('cargo', [`build`], {
  stdio: 'inherit',
})
const buildProcExit = await new Promise((resolve) => buildProc.on('exit', resolve))
if (buildProcExit !== 0) {
  process.exit(buildProcExit);
}

const proc = spawn('cargo', [`run`, `secure_js_sandbox_server`], {
  stdio: 'inherit',
  env: {
    ...process.env,
    SANDBOX_HTTP_MODE: "ALLOW_ALL",
    SANDBOX_TYPESCRIPT_SUPPORT: "true",
    SANDBOX_ENABLE_STRIP_TYPES_ENDPOINT: "true",
    SANDBOX_USE_MODULE_SYNTAX: "true",
  }
})

const timeout = Date.now() + 10_000;
let lastLog = Date.now();
let successCount = 0;
while (timeout > Date.now() && successCount < 10) {
  try {
    const res = await fetch('http://localhost:3000');
    if (res.ok) {
      successCount++;
    }
  } catch (e) {
    // ignore
  }
  if (Date.now() - lastLog > 1_000) {
    console.log('Waiting for server to start...');
    lastLog = Date.now();
  }
}

async function run({script, args}: {script: string, args: any[]}) {
  const response = await fetch('http://localhost:3000/evaluate', {
    method: 'POST',
    body: JSON.stringify({ script, args }),
    headers: { 'Content-Type': 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status} body: ${await response.text()}`);
  }
  return response.json();
}

async function runModule({code, method, args}: {code: string, method: string, args: any[]}) {
  const response = await fetch('http://localhost:3000/evaluate', {
    method: 'POST',
    body: JSON.stringify({ code, method, args }),
    headers: { 'Content-Type': 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status} body: ${await response.text()}`);
  }
  return response.json();
}


async function stripTypes(script: string) {
  const response = await fetch('http://localhost:3000/strip_types', {
    method: 'POST',
    body: JSON.stringify({ script }),
    headers: { 'Content-Type': 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status} body: ${await response.text()}`);
  }
  const outcome = await response.json();
  if (!outcome.success) {
    throw new Error(`Strip types failed: ${outcome.error}`);
  }
  return outcome.script;
}

console.log(await runModule({
  code: `import {fib} from 'http://localhost:3001/fib.js';
  export async function run(n: number) {
    return fib(n);
  }`,
  method: "run",
  args: [10]
}))


// console.log(await run({
//   script: `async () => {
//     const src = await fetch('http://localhost:3001/fib.js').then(res => res.text());
//     const exports = {};
//     new Function('exports', src.replace(/export function (\\w+)/, 'exports.$1 = function $1'))(exports);
//     return exports.fib(10);
//   }`,
//   args: []
// }))

// console.log(await run({
//   script: `async function fib(n: number) {
//     if (n <= 1) return n;
//     return await fib(n - 1) + await fib(n - 2);
//   }`,
//   args: [10]
// }))

console.log(
  await stripTypes(
  `
    async function fib(n: number) {
      if (n <= 1) return n;
      return await fib(n - 1) + await fib(n - 2);
    }
    `
  )
);


// console.log(await run({
//   script: `async function run() {
//     await fetch("custom-protocol://some/path");
//   }`,
//   args: []
// }))

proc.kill();
server.close();
