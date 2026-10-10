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
  watchersForTarget,
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

test("macOS usa captura nativa y Windows conserva ambos watchers", () => {
  assert.deepEqual(watchersForTarget("macos-arm64"), []);
  assert.deepEqual(watchersForTarget("macos-x86_64"), []);
  assert.deepEqual(watchersForTarget("windows-x86_64"), ["aw-watcher-window", "aw-watcher-afk"]);
});

test("parsea dry-run sin cambiar el target explícito", () => {
  const parsed = parseArgs(["--target", "x86_64-apple-darwin", "--dry-run"]);
  assert.equal(parsed.target, "macos-x86_64");
  assert.equal(parsed.dryRun, true);
  assert.throws(() => parseArgs(["--target"]), /Falta el valor/);
  assert.equal(parseArgs(["--help"]).help, true);
});

test("los artefactos fijados tienen checksums SHA-256 completos", () => {
  const publishedChecksums = {
    "macos-arm64": "e70639b63df662e1c72b4277e073bd80f7b94c516f12a8d01404f4aa526e8731",
    "macos-x86_64": "eb9bf65f2a0e28489b4f89eb64159a15b82f8f07d6eb6cd3efe0dd3565e61b36",
    "windows-x86_64": "45ce0ccc9e12ff60358878d3b8bef06da86957e53fc564ea2316cf2f190eb659",
  };
  for (const [target, artifact] of Object.entries(TARGETS)) {
    assert.match(target, /^(macos-(arm64|x86_64)|windows-x86_64)$/);
    assert.match(artifact.sha256, /^[a-f0-9]{64}$/);
    assert.match(artifact.archive, new RegExp(ACTIVITYWATCH_VERSION.replaceAll(".", "\\.")));
    assert.equal(artifact.sha256, publishedChecksums[target]);
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
    assert.equal(manifest.schemaVersion, 2);
    assert.equal(manifest.windowHelper, null);
    assert.equal(manifest.files.length, 1);
    assert.equal(manifest.files[0].path, "aw-watcher-afk/aw-watcher-afk");
    assert.equal(manifest.files[0].bytes, 7);
    assert.match(manifest.files[0].sha256, /^[a-f0-9]{64}$/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
