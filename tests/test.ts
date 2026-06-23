// node --experimental-strip-types tests/test.ts

import assert, { deepStrictEqual as eq } from "node:assert";
import { join } from "node:path";
import { createServer } from "node:http";
import { spawn } from "node:child_process";

interface EvaluateResult {
  fuel_consumed: number;
  fuel_remaining: number;
  max_requested_memory_bytes: number;
  max_requested_table_elements: number;
  outbound_requests: {
    outcome: "ALLOWED" | "BLOCKED";
    socket_addr: string | null;
    uri: string;
  }[];
  result: any;
  stderr: string;
  stdout: string;
  success: boolean;
}
type EvaluateResultMetricKeys =
  | "fuel_consumed"
  | "fuel_remaining"
  | "max_requested_memory_bytes"
  | "max_requested_table_elements";

type StripTypesResult =
  | { success: true; code: string }
  | { success: false; error: string };
type ValidateModuleResult =
  | {
      success: true;
      has_dynamic_import: boolean;
      static_imports: {
        source: string;
        imported_names: string[];
        has_star_import: boolean;
      }[];
      exports: ({ type: "NAMED"; name: string } | { type: "STAR"; source: string })[];
    }
  | { success: false; error: string };
async function killPortUser(port: number) {
  const killProc = spawn("zsh", ["-c", `lsof -t -i tcp:${port} | xargs kill -9`], {
    stdio: "inherit",
  });
  await new Promise(resolve => killProc.on("exit", resolve));
}
await killPortUser(3000);
await killPortUser(3001);
await killPortUser(3002);

const server = createServer((req, res) => {
  if (req.headers.host !== "127.0.0.1:3001") {
    eq(req.headers.host, "localhost:3001");
  }

  // console.log(req.method, req.url);
  if (req.url === "/fib.js") {
    res.writeHead(200, { "Content-Type": "text/javascript" });
    res.end(
      `
      export function fib(n) {
        if (n <= 1) return n;
        return fib(n - 1) + fib(n - 2);
      }
    `.trim(),
    );
    return;
  }
  if (req.url === "/to-redirect") {
    res.writeHead(302, { Location: "http://localhost:3002/from-redirect" });
    res.end("Redirecting");
    return;
  }
  res.writeHead(404);
  res.end("Not Found");
});
server.listen(3001);

const redirectDestinationServer = createServer((req, res) => {
  eq(req.headers.host, "localhost:3002");
  if (req.url === "/from-redirect") {
    res.writeHead(200, { "Content-Type": "text/plain" });
    res.end("from-redirect");
    return;
  }
  res.writeHead(404);
  res.end("Not Found");
});
redirectDestinationServer.listen(3002);

const buildProc = spawn("cargo", [`build`], {
  stdio: "inherit",
});
const buildProcExit = await new Promise<number>(resolve =>
  buildProc.on("exit", resolve),
);
if (buildProcExit !== 0) {
  process.exit(buildProcExit);
}

let proc: ReturnType<typeof spawn> | undefined;
async function startServer(env: Record<string, string>) {
  if (proc) {
    proc.kill();
  }

  proc = spawn("cargo", [`run`, `secure_js_sandbox_server`], {
    stdio: "inherit",
    env: {
      ...process.env,
      ...env,
    },
  });

  const timeout = Date.now() + 10_000;
  let lastLog = Date.now();
  let successCount = 0;
  while (timeout > Date.now() && successCount < 10) {
    try {
      const res = await fetch("http://localhost:3000");
      if (res.ok) {
        successCount++;
      }
    } catch (e) {
      // ignore
    }
    if (Date.now() - lastLog > 1_000) {
      console.log("Waiting for server to start...");
      lastLog = Date.now();
    }
  }
}

async function run({
  filename,
  code,
  parameters,
}: {
  filename?: string;
  code: string;
  parameters: any[];
}): Promise<EvaluateResult> {
  const response = await fetch("http://localhost:3000/evaluate", {
    method: "POST",
    body: JSON.stringify({ filename, code, parameters }),
    headers: { "Content-Type": "application/json" },
  });
  if (!response.ok) {
    throw new Error(
      `HTTP error! status: ${response.status} body: ${await response.text()}`,
    );
  }
  return response.json();
}

async function stripTypes(code: string): Promise<StripTypesResult> {
  const response = await fetch("http://localhost:3000/strip_types", {
    method: "POST",
    body: JSON.stringify({ code, filename: "input.ts" }),
    headers: { "Content-Type": "application/json" },
  });
  if (!response.ok) {
    throw new Error(
      `HTTP error! status: ${response.status} body: ${await response.text()}`,
    );
  }
  return await response.json();
}
async function validateModule(input: {
  filename?: string;
  code: string;
  mode?: "JAVASCRIPT" | "TYPESCRIPT";
}): Promise<ValidateModuleResult> {
  const response = await fetch("http://localhost:3000/validate_module", {
    method: "POST",
    body: JSON.stringify(input),
    headers: { "Content-Type": "application/json" },
  });
  if (!response.ok) {
    throw new Error(
      `HTTP error! status: ${response.status} body: ${await response.text()}`,
    );
  }
  const outcome = await response.json();
  return outcome;
}
async function expectRun(
  input: { filename?: string; code: string; parameters: any[] },
  expected: Omit<EvaluateResult, EvaluateResultMetricKeys>,
) {
  const {
    fuel_consumed,
    fuel_remaining,
    max_requested_memory_bytes,
    max_requested_table_elements,
    ...result
  } = await run(input);
  result.outbound_requests.forEach(req => {
    req.socket_addr = req.socket_addr
      ? req.socket_addr.replace(/^127\.0\.0\.1\:/, `[::1]:`)
      : null;
  });
  eq(result, expected);
}
async function expectStripTypes(input: string, expected: StripTypesResult) {
  const result = await stripTypes(input);
  eq(result, expected);
}
async function expectValidateModule(
  input: { filename?: string; code: string; mode?: "JAVASCRIPT" | "TYPESCRIPT" },
  expected: ValidateModuleResult,
) {
  const result = await validateModule(input);
  eq(result, expected);
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
});

console.log(
  await run({
    code: `
      export async function run(a: number, b: number) {
        return a + b;
      }
    `,
    parameters: [40, 2],
  }),
);

await expectRun(
  {
    code: `
      export async function run(a: number, b: number) {
        return a + b;
      }
    `,
    parameters: [40, 2],
  },
  {
    outbound_requests: [],
    result: 42,
    stderr: "",
    stdout: "",
    success: true,
  },
);

const fib = (n: number): number => {
  let a = 0,
    b = 1;
  for (let i = 0; i < n; i++) {
    [a, b] = [b, a + b];
  }
  return a;
};
for (let n = 0; n <= 30; n++) {
  const start = Date.now();
  await expectRun(
    {
      code: `
        // fib(n) = fib(n - 1) + fib(n - 2)
        export function run(n: number) {
          if (n <= 1) return n;
          return run(n - 1) + run(n - 2);
        }
      `,
      parameters: [n],
    },
    n < 25
      ? {
          outbound_requests: [],
          result: fib(n),
          stderr: "",
          stdout: "",
          success: true,
        }
      : {
          outbound_requests: [],
          result: {
            error: "CPU fuel exhausted",
          },
          stderr: "",
          stdout: "",
          success: false,
        },
  );
  const duration = Date.now() - start;

  assert(duration < 500, `n=${n} took too long: ${duration.toLocaleString()}ms`);
}
{
  const start = Date.now();
  await expectRun(
    {
      code: `
        export function run() {
          while (true);
        }
      `,
      parameters: [],
    },
    {
      outbound_requests: [],
      result: {
        error: "CPU fuel exhausted",
      },
      stderr: "",
      stdout: "",
      success: false,
    },
  );
  const duration = Date.now() - start;

  assert(duration < 500, `while (true); took too long: ${duration.toLocaleString()}ms`);
}

await expectRun(
  {
    code: `
        export function run() {
          const data = [];
          while (true) data.push(new Uint8Array(1024 * 1024));
        }
      `,
    parameters: [],
  },
  {
    outbound_requests: [],
    result: {
      error: "JavaScript error: out of memory",
    },
    stderr: "",
    stdout: "",
    success: false,
  },
);

{
  const { success, stdout, stderr, result } = await run({
    filename: "error_test.ts",
    code: `
      export async function run() {
        console.log("Attempting to generate output");
        console.error("This is going to throw");
        throw new Error("Hello World".repeat(42));
      }
    `,
    parameters: [],
  });
  eq(success, false);
  eq(stdout, "Attempting to generate output\n");
  eq(stderr, "This is going to throw\n");
  eq(result.error.startsWith(`JavaScript error: ${"Hello World".repeat(42)}`), true);
}

await expectRun(
  {
    filename: "error_test.ts",
    code: `
      export async function run() {
        await a();
      }
      async function a() {
        await new Promise(r => setTimeout(r, 10));
        await b();
      }
      async function b() {
        await c();
      }
      async function c() {
        throw new Error("Hello World");
      }
    `,
    parameters: [],
  },
  {
    outbound_requests: [],
    result: {
      error: [
        `JavaScript error: Hello World`,
        `  c@error_test.ts:15:15`,
        `  b@error_test.ts:12:15`,
        `  a@error_test.ts:9:15`,
        `  async*run@error_test.ts:5:15`,
      ].join("\n"),
    },
    stderr: "",
    stdout: "",
    success: false,
  },
);

// Check we can follow redirects
await expectRun(
  {
    code: `
          export async function run() {
            return await (await fetch('http://localhost:3001/to-redirect')).text()
          }
        `,
    parameters: [],
  },
  {
    outbound_requests: [
      {
        outcome: "ALLOWED",
        socket_addr: "[::1]:3001",
        uri: "http://localhost:3001/to-redirect",
      },
      {
        outcome: "ALLOWED",
        socket_addr: "[::1]:3002",
        uri: "http://localhost:3002/from-redirect",
      },
    ],
    result: "from-redirect",
    stderr: "",
    stdout: "",
    success: true,
  },
);

{
  let requestCount = 0;
  console.log("Testing redirect following under heavy load");
  await Promise.all(
    Array.from({ length: 100 }).map(async () => {
      for (let i = 0; i < 100; i++) {
        const { result } = await run({
          code: `
              export async function run() {
                let first = await (await fetch('http://localhost:3001/to-redirect')).text()
                for (let i = 0; i < 9; i++) {
                  if (first !== await (await fetch('http://localhost:3001/to-redirect')).text()) {
                    return "Inconsistent results"
                  }
                }
                return first
              }
            `,
          parameters: [],
        });
        eq(result, "from-redirect");
        requestCount++;
        if (0 === requestCount % 100) {
          process.stdout.write(".");
        }
      }
    }),
  );
  process.stdout.write("\n");
  console.log("Tested redirect following");
}

// Check IP addresses are allowed as well as DNS names
await expectRun(
  {
    code: `
      export async function run() {
        return await (await fetch('http://127.0.0.1:3001/to-redirect')).text()
      }
    `,
    parameters: [],
  },
  {
    outbound_requests: [
      {
        outcome: "ALLOWED",
        socket_addr: "[::1]:3001",
        uri: "http://127.0.0.1:3001/to-redirect",
      },
      {
        outcome: "ALLOWED",
        socket_addr: "[::1]:3002",
        uri: "http://localhost:3002/from-redirect",
      },
    ],
    result: "from-redirect",
    stderr: "",
    stdout: "",
    success: true,
  },
);

await expectRun(
  {
    code: `
      import { fib } from 'http://localhost:3001/fib.js';
      export async function run(n: number) {
        return fib(n);
      }
    `,
    parameters: [10],
  },
  {
    outbound_requests: [
      {
        outcome: "ALLOWED",
        socket_addr: "[::1]:3001",
        uri: "http://localhost:3001/fib.js",
      },
    ],
    result: 55,
    stderr: "",
    stdout: "",
    success: true,
  },
);

await expectRun(
  {
    code: `
      export async function run(n: number) {
        const { fib } = await import('http://localhost:3001/fib.js');
        return fib(n);
      }
    `,
    parameters: [10],
  },
  {
    outbound_requests: [
      {
        outcome: "ALLOWED",
        socket_addr: "[::1]:3001",
        uri: "http://localhost:3001/fib.js",
      },
    ],
    result: 55,
    stderr: "",
    stdout: "",
    success: true,
  },
);

{
  const { success, stdout, stderr, result } = await run({
    code: `
      import fib from 'http://localhost:3001/fib.js';
      export async function run(n: number) {
        return fib(n);
      }
    `,
    parameters: [10],
  });

  eq(success, false);
  eq(stdout, "");
  eq(stderr, "");
  eq(
    result.error.includes(
      "Module <main> tried to import 'default' from module 'http://localhost:3001/fib.js', but it does not export that name.",
    ),
    true,
  );
}

{
  const { success, stdout, stderr, result } = await run({
    code: `
      export async function run(n: number) {
        const { default: fib } = await import('http://localhost:3001/fib.js');
        return fib(n);
      }
    `,
    parameters: [10],
  });
  eq(success, false);
  eq(stdout, "");
  eq(stderr, "");
  eq(result.error.includes("fib is not a function"), true);
}

{
  const { success, stdout, stderr, result, outbound_requests } = await run({
    code: `
      export async function run() {
        for (let i = 0; i < 1_000_000; i++) {
          const res = await fetch("http://localhost:3001/fib.js");
          await res.text();
        }
      }
    `,
    parameters: [],
  });
  eq(success, false);
  eq(stdout, "");
  eq(stderr, "");
  eq(result.error.includes("NetworkError when attempting to fetch resource"), true);
  eq(outbound_requests.pop(), {
    outcome: "BLOCKED",
    socket_addr: null,
    uri: "http://localhost:3001/fib.js",
  });
  // Before the blocked request, there should be 10 allowed requests
  eq(outbound_requests.length, 10);
  for (const req of outbound_requests) {
    eq(req, {
      outcome: "ALLOWED",
      socket_addr: req.socket_addr || "[::1]:3001",
      uri: "http://localhost:3001/fib.js",
    });
  }
}

const stripTypesInput = `async function fib(n: number) {
  if (n <= 1) return n;
  return await fib(n - 1) + await fib(n - 2);
}`;
const stripTypesOutput = `async function fib(n        ) {
  if (n <= 1) return n;
  return await fib(n - 1) + await fib(n - 2);
}`;

await expectStripTypes(stripTypesInput, { success: true, code: stripTypesOutput });
// TODO: show better error here
const expectedStripTypesError = `error[InvalidSyntax]: Expected ';', '}' or <eof>
 --> input.ts:1:5
  |
1 | fun fib(n: number) -> n
  | --- ^^^
  | |
  | This is the expression part of an expression statement`;
await expectStripTypes(`fun fib(n: number) -> n`, {
  success: false,
  error: expectedStripTypesError,
});

await expectValidateModule(
  {
    code: `
      import fib from "http://example.com/fib.js";
      export async function run(n) {
        return fib(n);
      }
    `,
  },
  {
    success: true,
    has_dynamic_import: false,
    static_imports: [
      {
        source: "http://example.com/fib.js",
        imported_names: ["default"],
        has_star_import: false,
      },
    ],
    exports: [{ type: "NAMED", name: "run" }],
  },
);
await expectValidateModule(
  {
    code: `
      export async function run(n) {
        const { fib } = await import("http://example.com/fib.js");
        return fib(n);
      }
    `,
  },
  {
    success: true,
    has_dynamic_import: true,
    static_imports: [],
    exports: [{ type: "NAMED", name: "run" }],
  },
);
await expectValidateModule(
  {
    code: `
      export * from "http://example.com/fib.js";
    `,
  },
  {
    success: true,
    has_dynamic_import: false,
    static_imports: [
      {
        source: "http://example.com/fib.js",
        imported_names: [],
        has_star_import: true,
      },
    ],
    exports: [{ type: "STAR", source: "http://example.com/fib.js" }],
  },
);
await expectValidateModule(
  {
    code: `
      import fib from "http://example.com/fib.js";
      export async function run(n: number) {
        return fib(n);
      }
    `,
    mode: "TYPESCRIPT",
  },
  {
    success: true,
    has_dynamic_import: false,
    static_imports: [
      {
        source: "http://example.com/fib.js",
        imported_names: ["default"],
        has_star_import: false,
      },
    ],
    exports: [{ type: "NAMED", name: "run" }],
  },
);
await expectValidateModule(
  {
    filename: "test.js",
    code: `
      import fib from "http://example.com/fib.js";
      export async function run(n: number) {
        return fib(n);
      }
    `,
  },
  {
    success: false,
    error:
      "error: Expected ',', got ':'\n" +
      " --> test.js:3:34\n" +
      "  |\n" +
      "3 |       export async function run(n: number) {\n" +
      "  |                                  ^",
  },
);

await startServer({
  SANDBOX_HTTP_MODE: "ALLOW_ALL",
  SANDBOX_IMPORT_MAP_PATH: join(import.meta.dirname, "imports", "import-map.json"),
  SANDBOX_MODULE_METHOD: "run",
});

await expectRun(
  {
    code: `
      import { fib } from 'fib';
      export async function run(n) {
        return fib(n);
      }
    `,
    parameters: [10],
  },
  {
    outbound_requests: [],
    success: true,
    result: 55,
    stderr: "",
    stdout: "",
  },
);
await expectRun(
  {
    code: `
      import { fib } from 'fib-external';
      export async function run(n) {
        return fib(n);
      }
    `,
    parameters: [10],
  },
  {
    outbound_requests: [
      {
        outcome: `ALLOWED`,
        socket_addr: `[::1]:3001`,
        uri: `http://localhost:3001/fib.js`,
      },
    ],
    success: true,
    result: 55,
    stderr: "",
    stdout: "",
  },
);

await startServer({ SANDBOX_AUTO_STRIP_TYPES: "true" });
await expectRun(
  {
    code: `
      function add(a: number, b: number) {
        return a + b;
      }
    `,
    parameters: [40, 2],
  },
  {
    outbound_requests: [],
    result: 42,
    stderr: "",
    stdout: "",
    success: true,
  },
);
await expectRun(
  {
    code: `(a: number, b: number) => a + b`,
    parameters: [40, 2],
  },
  {
    outbound_requests: [],
    result: 42,
    stderr: "",
    stdout: "",
    success: true,
  },
);

await startServer({});

const THREE_MB = 3 * 1024 * 1024;
{
  const response = await fetch("http://localhost:3000/evaluate", {
    method: "POST",
    body: JSON.stringify({
      code: `(input) => input.length`,
      parameters: ["1".repeat(THREE_MB)],
    }),
    headers: { "Content-Type": "application/json" },
  });
  eq(response.status, 413);
  eq(await response.text(), "Failed to buffer the request body: length limit exceeded");
}

await startServer({ SANDBOX_API_REQUEST_BODY_LIMIT_BYTES: "4MB" });
await expectRun(
  {
    code: `(input) => input.length`,
    parameters: ["1".repeat(THREE_MB)],
  },
  {
    outbound_requests: [],
    result: THREE_MB,
    stderr: "",
    stdout: "",
    success: true,
  },
);

if (proc) proc.kill();
server.close();
redirectDestinationServer.close();
