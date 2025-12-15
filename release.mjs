import { execSync, spawnSync } from "node:child_process";
import { createInterface } from "node:readline/promises";

const existingTags = await (
  await fetch(
    "https://hub.docker.com/v2/repositories/forbeslindesay/secure-js-sandbox/tags",
  )
).json();
console.log(`Existing release tags:`);
console.log(existingTags.results.map(r => r.name).join("\n"));
const rl = createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: true,
});

const version = await rl.question("What is the new version number? ");
rl.close();

if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`Version number ${version} is not valid! Aborting.`);
  process.exit(1);
}

if (existingTags.results.some(r => r.name === `v${version}`)) {
  console.error(`A release for version ${version} already exists! Aborting.`);
  process.exit(1);
}

// execSync(`git tag v${version}`);
// execSync(`git push origin v${version}`);

console.log(`Building wasm sandbox...`);

const buildExit = spawnSync(`node`, [`--run`, `build:release`], { stdio: "inherit" });
if (buildExit.status !== 0) {
  process.exit(buildExit.status || 1);
}

console.log(`Releasing version ${version}...`);

const dockerExit = spawnSync(
  `docker`,
  [
    `buildx`,
    `build`,
    `--tag`,
    `forbeslindesay/secure-js-sandbox:latest`,
    `--tag`,
    `forbeslindesay/secure-js-sandbox:v${version}`,
    `--platform`,
    `linux/arm64,linux/amd64`,
    `--builder`,
    `container`,
    `--push`,
    `.`,
  ],
  { stdio: "inherit" },
);
if (dockerExit.status !== 0) {
  process.exit(dockerExit.status || 1);
}
