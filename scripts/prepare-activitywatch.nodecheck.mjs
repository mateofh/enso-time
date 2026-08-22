import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  ACTIVITYWATCH_VERSION,
  buildManifest,
  detectTarget,
  normalizeTarget,
  parseArgs,
  TARGETS,
} from "./prepare-activitywatch.mjs";

test("normaliza triples de Tauri y targets canónicos", () => {
  assert.equal(normalizeTarget("aarch64-apple-darwin"), "macos-arm64");
  assert.equal(normalizeTarget("x86_64-apple-darwin"), "macos-x86_64");
  assert.equal(normalizeTarget("x86_64-pc-windows-msvc"), "windows-x86_64");
  assert.equal(normalizeTarget("macos-arm64"), "macos-arm64");
  assert.throws(() => normalizeTarget("linux-x86_64"), /Target no soportado/);
});

test("detecta las tres plataformas soportadas", () => {
  assert.equal(detectTarget("darwin", "arm64"), "macos-arm64");
  assert.equal(detectTarget("darwin", "x64"), "macos-x86_64");
  assert.equal(detectTarget("win32", "x64"), "windows-x86_64");
  assert.throws(() => detectTarget("linux", "x64"), /Plataforma no soportada/);
});

test("parsea dry-run sin cambiar el target explícito", () => {
  const parsed = parseArgs(["--target", "x86_64-apple-darwin", "--dry-run"]);
  assert.equal(parsed.target, "macos-x86_64");
  assert.equal(parsed.dryRun, true);
  assert.throws(() => parseArgs(["--target"]), /Falta el valor/);
  assert.equal(parseArgs(["--help"]).help, true);
});

test("los artefactos fijados tienen checksums SHA-256 completos", () => {
  for (const [target, artifact] of Object.entries(TARGETS)) {
    assert.match(target, /^(macos-(arm64|x86_64)|windows-x86_64)$/);
    assert.match(artifact.sha256, /^[a-f0-9]{64}$/);
    assert.match(artifact.archive, new RegExp(ACTIVITYWATCH_VERSION.replaceAll(".", "\\.")));
  }
});

test("el manifiesto enumera y firma el runtime extraído", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "enso-aw-manifest-"));
  try {
    await mkdir(path.join(root, "aw-watcher-afk"));
    await writeFile(path.join(root, "aw-watcher-afk", "aw-watcher-afk"), "fixture");
    const manifest = await buildManifest(root, {
      archive: "fixture.zip",
      sha256: "a".repeat(64),
      source: "https://example.test/fixture.zip",
      target: "macos-arm64",
    });
    assert.equal(manifest.schemaVersion, 1);
    assert.equal(manifest.files.length, 1);
    assert.equal(manifest.files[0].path, "aw-watcher-afk/aw-watcher-afk");
    assert.equal(manifest.files[0].bytes, 7);
    assert.match(manifest.files[0].sha256, /^[a-f0-9]{64}$/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
