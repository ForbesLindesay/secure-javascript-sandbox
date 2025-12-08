// node --experimental-strip-types tests/test.ts

import { join } from 'node:path';
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
const buildProcExit = await new Promise<number>((resolve) => buildProc.on('exit', resolve))
if (buildProcExit !== 0) {
  process.exit(buildProcExit);
}

let proc: ReturnType<typeof spawn> | undefined;
async function startServer(env: Record<string, string>) {
  if (proc) {
    proc.kill();
  }

  proc = spawn('cargo', [`run`, `secure_js_sandbox_server`], {
    stdio: 'inherit',
    env: {
      ...process.env,
      ...env,
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
}

async function run({code, parameters}: {code: string, parameters: any[]}) {
  const response = await fetch('http://localhost:3000/evaluate', {
    method: 'POST',
    body: JSON.stringify({ code, parameters }),
    headers: { 'Content-Type': 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status} body: ${await response.text()}`);
  }
  return response.json();
}

async function stripTypes(code: string) {
  const response = await fetch('http://localhost:3000/strip_types', {
    method: 'POST',
    body: JSON.stringify({ code }),
    headers: { 'Content-Type': 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status} body: ${await response.text()}`);
  }
  const outcome = await response.json();
  if (!outcome.success) {
    throw new Error(`Strip types failed: ${outcome.error}`);
  }
  return outcome.code;
}
async function validateModule(code: string, mode?: "JAVASCRIPT" | "TYPESCRIPT") {
  const response = await fetch('http://localhost:3000/validate_module', {
    method: 'POST',
    body: JSON.stringify({ code, mode }),
    headers: { 'Content-Type': 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status} body: ${await response.text()}`);
  }
  const outcome = await response.json();
  return outcome;
}

await startServer({
  SANDBOX_HTTP_MODE: "ALLOW_ALL",
  SANDBOX_AUTO_STRIP_TYPES: "true",
  SANDBOX_ENABLE_STRIP_TYPES_ENDPOINT: "true",
  SANDBOX_ENABLE_VALIDATE_MODULE_ENDPOINT: "true",
  SANDBOX_MODULE_METHOD: "run",
  SANDBOX_REQUEST_LIMIT: "10",
  TS_UTILS_CPU_FUEL: `1_000_000`,
  TS_UTILS_MAX_MEMORY_BYTES: `10MB`,
})

console.log(await run({
  code: `
  export async function run(a: number, b: number) {
    return a + b;
  }`,
  parameters: [40, 2]
}))

console.log(await run({
  code: `
  export async function run() {
    console.log("Attempting to generate output");
    console.error("This is going to throw");
    throw new Error("Hello World".repeat(42));
  }`,
  parameters: []
}))

// console.log(await run({
//   code: `
//   export async function run() {
//     for (let i = 0; i < 100; i++) {
//       const res = await fetch('http://localhost:3001/fib.js');
//       const data = await res.text();
//     }
// }`,  parameters: []
// }))

console.log(await run({
  code: `import { fib } from 'http://localhost:3001/fib.js';
  export async function run(n: number) {
    return fib(n);
  }`,
  parameters: [10]
}))
console.log(await run({
  code: `// import {fib} from 'http://localhost:3001/fib.js';
  export async function run(n: number) {
    const { fib } = await import('http://localhost:3001/fib.js');
    return fib(n);
  }`,
  parameters: [10]
}))

console.log(await run({
  code: `// import {fib} from 'http://localhost:3001/fib.js';
  export async function run(n: number) {
    const { default: fib } = await import('http://localhost:3001/fib.js');
    return fib(n);
  }`,
  parameters: [10]
}))

console.log(await run({
  code: `export async function run() {
    for (let i = 0; i < 1_000_000; i++) {
      const res = await fetch("http://example.com");
      await res.text();
    }
  }`,
  parameters: []
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
console.log(
  await validateModule(
  `
      import fib from "http://example.com/fib.js";
      export async function run(n) {
        return fib(n);
      }
    `
  )
);
console.log(
  await validateModule(
  `
      import * as x from "/x.js";
      import fib from "http://example.com/fib.js";
      export async function run(n: number) {
        return fib(n);
      }
      export * from "/y.js";
    `,
    "TYPESCRIPT"
  )
);
console.log(
  await validateModule(
  `
      import fib from "http://example.com/fib.js";
      export async function run(n: number) {
        return fib(n);
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


await startServer({
  SANDBOX_HTTP_MODE: "ALLOW_ALL",
  SANDBOX_IMPORT_MAP_PATH: join(import.meta.dirname, 'imports', 'import-map.json'),
  SANDBOX_MODULE_METHOD: "run",
})

console.log(await run({
  code: `import { fib } from 'fib';
  export async function run(n) {
    return fib(n);
  }`,
  parameters: [10]
}))

console.log(await run({
  code: `import { fib } from 'fib-external';
  export async function run(n) {
    return fib(n);
  }`,
  parameters: [10]
}))


proc.kill();
server.close();
