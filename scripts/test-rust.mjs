import { spawnSync } from "node:child_process";
import { delimiter, dirname } from "node:path";

// rustup may be on PATH while its toolchain binaries are not (e.g. Homebrew).
const located = spawnSync("rustup", ["which", "cargo"], { encoding: "utf8" });
const cargo = located.status === 0 ? located.stdout.trim() : "cargo";
const env = { ...process.env };
if (located.status === 0) {
  const pathKey = Object.keys(env).find((key) => key.toLowerCase() === "path") ?? "PATH";
  env[pathKey] = `${dirname(cargo)}${delimiter}${env[pathKey] ?? ""}`;
}
const args = ["test", "--manifest-path", "src-tauri/Cargo.toml", "--locked"];
if (process.env.RUST_TARGET) args.push("--target", process.env.RUST_TARGET);
args.push("--", "--test-threads=1");
const result = spawnSync(cargo, args, { env, stdio: "inherit" });
if (result.error) console.error(`Rust checks could not start: ${result.error.message}. Install the stable Rust toolchain.`);
process.exitCode = result.status ?? 1;
