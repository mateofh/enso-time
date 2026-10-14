import { spawn } from "node:child_process";
import { mkdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const value = (flag) => process.argv[process.argv.indexOf(flag) + 1];
if (!process.argv.includes("--app") || !process.argv.includes("--report")) throw new Error("Expected --app EXECUTABLE --report JSON");
const executable = resolve(value("--app"));
const reportPath = resolve(value("--report"));
mkdirSync(dirname(reportPath), { recursive: true });
const child = spawn(executable, ["--smoke-test", "--smoke-report", reportPath], { stdio: "inherit" });
let timedOut = false;
const timeout = setTimeout(() => {
  timedOut = true;
  child.kill();
}, 45_000);
const code = await new Promise((done, reject) => {
  child.on("error", reject);
  child.on("close", (status) => done(status));
}).finally(() => clearTimeout(timeout));
if (timedOut || code !== 0) throw new Error(`Installed app smoke failed (timeout=${timedOut}, exit=${code})`);
const report = JSON.parse(readFileSync(reportPath, "utf8"));
const { version } = JSON.parse(readFileSync("package.json", "utf8"));
if (report.status !== "passed" || report.version !== version || (process.env.SOURCE_COMMIT && report.sourceCommit !== process.env.SOURCE_COMMIT)) {
  throw new Error("Installed app smoke report does not match the built release");
}
for (const check of ["webview-initialization", "tauri-ipc", "local-server", "sqlite-read-write"]) {
  if (!report.checks?.includes(check)) throw new Error(`Missing installed app smoke check: ${check}`);
}
console.log("Installed app smoke passed; OS capture permissions still require manual testing.");
