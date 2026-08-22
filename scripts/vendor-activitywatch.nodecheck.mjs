import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const vendorRoot = path.join(projectRoot, "vendor", "activitywatch");
const expectedCommit = "7c33810cd44393b3ecad64277b7e69bfeaee817b";

test("el servidor vendorizado declara el commit upstream fijado", async () => {
  const commit = (await readFile(path.join(vendorRoot, "UPSTREAM_COMMIT"), "utf8")).trim();
  assert.equal(commit, expectedCommit);
});

test("están presentes únicamente los crates requeridos por el servidor", async () => {
  for (const crate of [
    "aw-server",
    "aw-client-rust",
    "aw-datastore",
    "aw-models",
    "aw-query",
    "aw-transform",
  ]) {
    await access(path.join(vendorRoot, crate, "Cargo.toml"));
  }
});

test("CORS de producción no permite orígenes de extensiones", async () => {
  const cors = await readFile(
    path.join(vendorRoot, "aw-server", "src", "endpoints", "cors.rs"),
    "utf8",
  );
  assert.match(cors, /let mut allowed_regex_origins = Vec::<String>::new\(\);/);
  assert.doesNotMatch(cors, /moz-extension:\/\/\.\*/);
  assert.doesNotMatch(cors, /nglaklhklhcoonedhgnpgddginnjdadi/);
});

test("el web UI upstream se sustituye por el placeholder mínimo", async () => {
  const placeholder = await readFile(
    path.join(vendorRoot, "aw-server", "server-static", "index.html"),
    "utf8",
  );
  assert.ok(placeholder.length < 1_000);
  assert.match(placeholder, /Ensō Time|Enso Time/);
});
