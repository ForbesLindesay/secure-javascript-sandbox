import { spawnSync } from "child_process";
import { createHash } from "crypto";
import { mkdirSync, readFileSync, writeFileSync } from "fs";
import { dirname, join } from "path";

function run(name: string, args: string[], { cwd, cache }: { cwd: string; cache?: {inputFiles: string[], outputFile: string} }) {
  if (cache) {
    const hash = createHash("sha512");
    hash.update(name + "\n");
    for (const arg of args) {
      hash.update(arg + "\n");
    }
    for (const inputFile of cache.inputFiles) {
      hash.update(inputFile + "\n");
      hash.update(readFileSync(inputFile));
      hash.update("\n");
    }
    const digest = hash.digest("hex");
    let existingHash: string | undefined;
    try {
      existingHash = readFileSync(cache.outputFile + ".cache", "utf8");
    } catch (ex: any) {
      if (ex.code !== "ENOENT") throw ex;
    }
    if (existingHash === digest) {
      console.log(`Skipping ${name} ${args.join(" ")} (cached)`);
    } else {
      run(name, args, { cwd });
      writeFileSync(cache.outputFile + ".cache", digest);
    }
    return;
  }
  console.log(`${name} ${args.join(" ")}`);
  const proc = spawnSync(name, args, {
    cwd,
    stdio: "inherit",
  })
  if (proc.status !== 0) {
    process.exit(1);
  }
}

const releaseMode = process.argv.includes("--release");

console.log(`Compiling ts_utils_wasm`);
run(
  "cargo",
  [
    "build",
    "--target",
    "wasm32-wasip2",
    "--package",
    "secure_js_sandbox_ts_utils_wasm",
    ...(releaseMode ? ["--release"] : []),
  ],
  {
    cwd: dirname(import.meta.dirname)
  }
);

const utilsWasmUnoptimized = join(dirname(import.meta.dirname), `target/wasm32-wasip2/${releaseMode ? "release" : "debug"}/secure_js_sandbox_ts_utils_wasm.wasm`);
if (releaseMode) {
  run(
    `npx`,
    [
      `jco`,
      `opt`,
      utilsWasmUnoptimized,
      `--output`, `tsutils.wasm`,
      // `--asyncify`,
    ],
    {
      cwd: join(import.meta.dirname, `build`),
      cache: {
        inputFiles: [utilsWasmUnoptimized,],
        outputFile: join(import.meta.dirname, `build/tsutils.wasm`),
      }
    },
  );
} else {
  writeFileSync(join(import.meta.dirname, `build/tsutils.wasm.cache`), "debug_build");
  writeFileSync(
    join(import.meta.dirname, `build/tsutils.wasm`),
    readFileSync(utilsWasmUnoptimized)
  );
}
const utilsWasm = join(import.meta.dirname, `build/tsutils.wasm`);
const utilsWit = readFileSync(join(dirname(import.meta.dirname), `crates/ts_utils_wasm/wit/ts-utils.wit`));

writeFileSync(
  join(dirname(import.meta.dirname), `crates/sandbox/src/tsutils.wasm`),
  readFileSync(utilsWasm)
);
writeFileSync(
  join(dirname(import.meta.dirname), `crates/sandbox/src/tsutils.wit`),
  utilsWit
);

mkdirSync(join(import.meta.dirname, `wit/deps`), { recursive: true });
writeFileSync(join(import.meta.dirname, `wit/deps/ts-utils.wit`), utilsWit);
run(
  `npx`,
  [
    `jco`,
    `componentize`,
    `--wit`,
    `wit/`,
    `--world-name`,
    `sandbox`,
    `--out`,
    `build/unbundled.wasm`,
    `input.js`
  ],
  {
    cwd: import.meta.dirname,
    cache: {
      inputFiles: [
        join(import.meta.dirname, `input.js`),
        join(import.meta.dirname, `wit/sandbox.wit`),
        join(import.meta.dirname, `wit/deps/ts-utils.wit`),
      ],
      outputFile: join(import.meta.dirname, `build/unbundled.wasm`),
    }
  }
);

run(
  `wac`,
  [
    `plug`,
    `unbundled.wasm`,
    `--plug`,
    utilsWasm,
    `--output`,
    `bundled.wasm`,
  ],
  {
    cwd: join(import.meta.dirname, `build`),
    cache: {
      inputFiles: [
        join(import.meta.dirname, `build/unbundled.wasm`),
        utilsWasm,
      ],
      outputFile: join(import.meta.dirname, `build/bundled.wasm`),
    }
  }
);

if (releaseMode) {
  run(
    `npx`,
    [
      `jco`,
      `opt`,
      `bundled.wasm`,
      `--output`, `final.wasm`,
      // `--asyncify`,
    ],
    {
      cwd: join(import.meta.dirname, `build`),
      cache: {
        inputFiles: [
          join(import.meta.dirname, `build/bundled.wasm`),
        ],
        outputFile: join(import.meta.dirname, `build/final.wasm`),
      }
    },
  );
} else {
  writeFileSync(join(import.meta.dirname, `build/final.wasm.cache`), "debug_build");
  writeFileSync(
    join(import.meta.dirname, `build/final.wasm`),
    readFileSync(join(import.meta.dirname, `build/bundled.wasm`))
  );
}
run(
  `npx`,
  [
    `jco`,
    `wit`,
    `final.wasm`,
    `--output`, `final.wit`,
    // `--asyncify`,
  ],
  { cwd: join(import.meta.dirname, `build`) },
);
writeFileSync(
  join(import.meta.dirname, `build/final.wit`),
  readFileSync(join(import.meta.dirname, `build/final.wit`), "utf8").split(`\n`).filter(line => !line.trim().startsWith(`import wasi:`)).join(`\n`)
);

console.log(`File sizes for shared world:`);
console.log(``);
for (const stage of [`unbundled`, `bundled`, `final`]) {
  console.log(`- ${stage}: ${readFileSync(join(import.meta.dirname, `build/${stage}.wasm`)).byteLength.toLocaleString()} bytes`);
}
console.log(``);

const rustSandboxDir = join(import.meta.dirname, `../crates/sandbox/src`);
writeFileSync(
  join(rustSandboxDir, `sandbox.wasm`),
  readFileSync(join(import.meta.dirname, `build/final.wasm`))
);
writeFileSync(
  join(rustSandboxDir, `sandbox.wit`),
  readFileSync(join(import.meta.dirname, `build/final.wit`))
);

console.log("DONE");