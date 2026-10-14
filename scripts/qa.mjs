import { spawn, spawnSync } from "node:child_process";
import { createWriteStream, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const directory = resolve(root, "node_modules/.cache/enso-time-qa");
mkdirSync(directory, { recursive: true });
const log = createWriteStream(resolve(directory, "checks.log"));
const git = (args) => spawnSync("git", args, { cwd: root, encoding: "utf8" });
const source = git(["rev-parse", "HEAD"]);
const tree = git(["status", "--porcelain"]);
const report = {
  version: JSON.parse(readFileSync(resolve(root, "package.json"), "utf8")).version,
  sourceCommit: source.status === 0 ? source.stdout.trim() : null,
  sourceTreeDirty: tree.status === 0 ? Boolean(tree.stdout.trim()) : null,
  platform: process.platform, architecture: process.arch,
  startedAt: new Date().toISOString(), status: "running", steps: [],
};
const npm = process.env.npm_execpath;
if (!npm) throw new Error("Run this command through npm run qa.");
const steps = [
  ["Application", [npm, "run", "check"]],
  ["ActivityWatch packaging", [npm, "run", "test:activitywatch"]],
  ["Release contracts", [npm, "run", "test:release"]],
  ["License boundaries", ["scripts/license-check.mjs", "--strict-manifests"]],
  ["License boundaries (CRLF)", ["scripts/license-check.mjs", "--strict-manifests", "--simulate-crlf"]],
  ["Rust", ["scripts/test-rust.mjs"]],
];
const save = () => writeFileSync(resolve(directory, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
save();
for (const [name, args] of steps) {
  console.log(`\nQA: ${name}`);
  const startedAt = new Date().toISOString();
  const code = await new Promise((resolveCode) => {
    const child = spawn(process.execPath, args, { cwd: root, env: process.env });
    child.stdout.on("data", (data) => { process.stdout.write(data); log.write(data); });
    child.stderr.on("data", (data) => { process.stderr.write(data); log.write(data); });
    child.on("error", (error) => { log.write(`${error.message}\n`); resolveCode(1); });
    child.on("close", (status) => resolveCode(status ?? 1));
  });
  report.steps.push({ name, startedAt, finishedAt: new Date().toISOString(), exitCode: code });
  save();
  if (code !== 0) { report.status = "failed"; process.exitCode = code; break; }
}
if (report.status === "running") report.status = "passed";
report.finishedAt = new Date().toISOString();
save();
await new Promise((done) => log.end(done));
console.log(`\nQA ${report.status}. Report: ${directory}/report.json`);
