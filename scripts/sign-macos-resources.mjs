import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { buildManifest, TARGETS } from "./prepare-activitywatch.mjs";

if (process.platform !== "darwin") throw new Error("macOS resource signing must run on macOS");
const identity = process.env.APPLE_SIGNING_IDENTITY;
if (!identity?.startsWith("Developer ID Application:")) throw new Error("Developer ID Application identity required");
const root = resolve("src-tauri/resources/activitywatch");
const original = JSON.parse(readFileSync(resolve(root, "manifest.json"), "utf8"));
if (TARGETS[original.target]?.sha256 !== original.sourceArchiveSha256) throw new Error("Unexpected upstream archive provenance");
let count = 0;
for (const entry of original.files) {
  const file = resolve(root, entry.path);
  if (!file.startsWith(`${root}/`)) throw new Error("Resource path escapes prepared directory");
  const type = spawnSync("file", ["-b", file], { encoding: "utf8" });
  if (type.status !== 0) throw new Error("Could not inspect resource type");
  if (!type.stdout.includes("Mach-O")) continue;
  const sign = spawnSync("codesign", ["--force", "--options", "runtime", "--timestamp", "--sign", identity, file], { stdio: "inherit" });
  if (sign.status !== 0) throw new Error(`Failed to sign resource: ${entry.path}`);
  const verify = spawnSync("codesign", ["--verify", "--strict", file], { stdio: "inherit" });
  if (verify.status !== 0) throw new Error(`Failed to verify signed resource: ${entry.path}`);
  count += 1;
}
if (count === 0) throw new Error("Prepared runtime contains no Mach-O resources");
// Signing changes binary bytes. Recompute the local integrity manifest while
// retaining the verified upstream archive hash and per-file runtime checks.
const manifest = await buildManifest(root, {
  source: original.source, archive: original.sourceArchive,
  sha256: original.sourceArchiveSha256, target: original.target,
});
writeFileSync(resolve(root, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Signed and verified ${count} prepared Mach-O resources; integrity manifest refreshed.`);
