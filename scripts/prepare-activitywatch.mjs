#!/usr/bin/env node

import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import {
  access,
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { pipeline } from "node:stream/promises";
import { spawn } from "node:child_process";

export const ACTIVITYWATCH_VERSION = "v0.14.0b3";
export const WATCHERS = ["aw-watcher-window", "aw-watcher-afk"];
const RELEASE_BASE = `https://github.com/ActivityWatch/activitywatch/releases/download/${ACTIVITYWATCH_VERSION}`;

export const TARGETS = Object.freeze({
  "macos-arm64": {
    archive: `activitywatch-${ACTIVITYWATCH_VERSION}-macos-arm64.zip`,
    sha256: "70a7ba7e307abe3352b912411f09600075f427b5b5d139a238c051f813e96264",
    executableSuffix: "",
  },
  "macos-x86_64": {
    archive: `activitywatch-${ACTIVITYWATCH_VERSION}-macos-x86_64.zip`,
    sha256: "eb9bf65f2a0e28489b4f89eb64159a15b82f8f07d6eb6cd3efe0dd3565e61b36",
    executableSuffix: "",
  },
  "windows-x86_64": {
    archive: `activitywatch-${ACTIVITYWATCH_VERSION}-windows-x86_64.zip`,
    sha256: "45ce0ccc9e12ff60358878d3b8bef06da86957e53fc564ea2316cf2f190eb659",
    executableSuffix: ".exe",
  },
});

const TARGET_ALIASES = Object.freeze({
  "aarch64-apple-darwin": "macos-arm64",
  "arm64-apple-darwin": "macos-arm64",
  "x86_64-apple-darwin": "macos-x86_64",
  "x64-apple-darwin": "macos-x86_64",
  "x86_64-pc-windows-msvc": "windows-x86_64",
  "x86_64-pc-windows-gnu": "windows-x86_64",
  "x64-pc-windows-msvc": "windows-x86_64",
});

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const defaultOutput = path.join(projectRoot, "src-tauri", "resources", "activitywatch");

export function normalizeTarget(value) {
  if (!value) return undefined;
  const normalized = value.trim().toLowerCase();
  const target = TARGET_ALIASES[normalized] ?? normalized;
  if (!Object.hasOwn(TARGETS, target)) {
    throw new Error(
      `Target no soportado: ${value}. Usa ${Object.keys(TARGETS).join(", ")}.`,
    );
  }
  return target;
}

export function detectTarget(platform = process.platform, architecture = process.arch) {
  if (platform === "darwin" && architecture === "arm64") return "macos-arm64";
  if (platform === "darwin" && architecture === "x64") return "macos-x86_64";
  if (platform === "win32" && architecture === "x64") return "windows-x86_64";
  throw new Error(`Plataforma no soportada: ${platform}/${architecture}. Indica --target.`);
}

export function parseArgs(argv) {
  const options = { archive: undefined, dryRun: false, force: false, output: defaultOutput, target: undefined };
  const nextValue = (index, option) => {
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`Falta el valor de ${option}`);
    return value;
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--dry-run") options.dryRun = true;
    else if (argument === "--force") options.force = true;
    else if (argument === "--help" || argument === "-h") options.help = true;
    else if (argument.startsWith("--target=")) {
      const value = argument.slice(9);
      if (!value) throw new Error("Falta el valor de --target");
      options.target = normalizeTarget(value);
    }
    else if (argument === "--target") options.target = normalizeTarget(nextValue(index++, argument));
    else if (argument.startsWith("--archive=")) options.archive = path.resolve(argument.slice(10));
    else if (argument === "--archive") options.archive = path.resolve(nextValue(index++, argument));
    else if (argument.startsWith("--output=")) options.output = path.resolve(argument.slice(9));
    else if (argument === "--output") options.output = path.resolve(nextValue(index++, argument));
    else throw new Error(`Argumento desconocido: ${argument}`);
  }
  if (!options.help) {
    options.target ??= normalizeTarget(process.env.TAURI_ENV_TARGET_TRIPLE) ?? detectTarget();
  }
  return options;
}

export async function sha256(filePath) {
  const hash = createHash("sha256");
  await pipeline(createReadStream(filePath), hash);
  return hash.digest("hex");
}

async function exists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function run(command, args) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: "inherit", windowsHide: true });
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} terminó con código ${code}`));
    });
  });
}

async function download(url, destination) {
  const temporary = `${destination}.${process.pid}.download`;
  await rm(temporary, { force: true });
  try {
    const response = await fetch(url, { redirect: "follow" });
    if (!response.ok || !response.body) {
      throw new Error(`No se pudo descargar ${url}: HTTP ${response.status}`);
    }
    await pipeline(response.body, createWriteStream(temporary, { mode: 0o600 }));
    await rename(temporary, destination);
  } catch (error) {
    await rm(temporary, { force: true });
    throw error;
  }
}

async function collectFiles(root, relative = "") {
  const directory = path.join(root, relative);
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    const child = path.join(relative, entry.name);
    if (entry.isDirectory()) files.push(...(await collectFiles(root, child)));
    else if (entry.isFile()) files.push(child);
  }
  return files;
}

export async function buildManifest(root, metadata) {
  const files = await collectFiles(root);
  const watcherFiles = [];
  for (const relativePath of files) {
    if (relativePath === "manifest.json") continue;
    const absolutePath = path.join(root, relativePath);
    const fileStat = await stat(absolutePath);
    watcherFiles.push({
      path: relativePath.split(path.sep).join("/"),
      bytes: fileStat.size,
      sha256: await sha256(absolutePath),
    });
  }
  return {
    schemaVersion: 1,
    activitywatchVersion: ACTIVITYWATCH_VERSION,
    source: metadata.source,
    sourceArchive: metadata.archive,
    sourceArchiveSha256: metadata.sha256,
    target: metadata.target,
    generatedAt: new Date().toISOString(),
    files: watcherFiles,
  };
}

async function validateInstall(root, target) {
  const suffix = TARGETS[target].executableSuffix;
  for (const watcher of WATCHERS) {
    const executable = path.join(root, watcher, `${watcher}${suffix}`);
    const executableStat = await stat(executable);
    if (!executableStat.isFile() || executableStat.size === 0) {
      throw new Error(`Ejecutable incompleto: ${executable}`);
    }
    if (!suffix) await chmod(executable, 0o755);
  }
}

async function currentInstallMatches(output, target) {
  try {
    const manifest = JSON.parse(await readFile(path.join(output, "manifest.json"), "utf8"));
    if (
      manifest.target !== target ||
      manifest.sourceArchiveSha256 !== TARGETS[target].sha256 ||
      manifest.activitywatchVersion !== ACTIVITYWATCH_VERSION
    ) return false;
    await validateInstall(output, target);
    if (!Array.isArray(manifest.files) || manifest.files.length === 0) return false;
    for (const file of manifest.files) {
      if (
        typeof file.path !== "string" ||
        file.path.startsWith("/") ||
        file.path.split("/").includes("..") ||
        typeof file.bytes !== "number" ||
        typeof file.sha256 !== "string"
      ) return false;
      const absolutePath = path.join(output, ...file.path.split("/"));
      const fileStat = await stat(absolutePath);
      if (fileStat.size !== file.bytes || await sha256(absolutePath) !== file.sha256) return false;
    }
    return true;
  } catch {
    return false;
  }
}

async function assertSafeOutput(output) {
  const resolved = path.resolve(output);
  const filesystemRoot = path.parse(resolved).root;
  const forbidden = new Set([
    filesystemRoot,
    path.resolve(homedir()),
    path.resolve(tmpdir()),
    projectRoot,
    path.dirname(projectRoot),
    path.join(projectRoot, "src-tauri", "resources"),
  ]);
  if (forbidden.has(resolved)) {
    throw new Error(`Directorio de salida inseguro: ${resolved}`);
  }
  if (await exists(resolved) && !(await exists(path.join(resolved, "manifest.json")))) {
    throw new Error(
      `No se reemplazará ${resolved}: no contiene un manifest.json generado por este script.`,
    );
  }
}

async function extractWatchers(archive, stagingDirectory) {
  const selectors = WATCHERS.map((watcher) => `activitywatch/${watcher}`);
  if (process.platform === "win32") {
    await run("tar", ["-xf", archive, "-C", stagingDirectory, ...selectors]);
    return;
  }
  await run("unzip", ["-q", "-o", archive, ...selectors.map((selector) => `${selector}/*`), "-d", stagingDirectory]);
}

function printHelp() {
  console.log(`Prepara los watchers nativos de ActivityWatch para Ensō Time.

Uso:
  node scripts/prepare-activitywatch.mjs [opciones]

Opciones:
  --target <target>   macos-arm64, macos-x86_64 o windows-x86_64
  --archive <ruta>   Usa un ZIP local (se verifica igualmente)
  --output <ruta>    Directorio de salida
  --force            Reextrae aunque el manifiesto actual coincida
  --dry-run          Muestra la operación sin descargar ni escribir
  --help              Muestra esta ayuda`);
}

export async function prepare(options) {
  const release = TARGETS[options.target];
  const url = `${RELEASE_BASE}/${release.archive}`;
  const sourceArchive = options.archive ?? path.join(
    tmpdir(),
    "enso-time",
    "activitywatch",
    ACTIVITYWATCH_VERSION,
    release.archive,
  );

  if (options.dryRun) {
    return { action: "dry-run", target: options.target, url, archive: sourceArchive, output: options.output };
  }

  await assertSafeOutput(options.output);

  if (!options.force && await currentInstallMatches(options.output, options.target)) {
    return { action: "unchanged", target: options.target, output: options.output };
  }

  if (!options.archive) {
    await mkdir(path.dirname(sourceArchive), { recursive: true });
    if (!(await exists(sourceArchive)) || (await sha256(sourceArchive)) !== release.sha256) {
      await rm(sourceArchive, { force: true });
      console.log(`Descargando ${url}`);
      await download(url, sourceArchive);
    }
  }

  const actualChecksum = await sha256(sourceArchive);
  if (actualChecksum !== release.sha256) {
    throw new Error(
      `Checksum inválido para ${release.archive}: esperado ${release.sha256}, recibido ${actualChecksum}`,
    );
  }

  const resourceParent = path.dirname(options.output);
  await mkdir(resourceParent, { recursive: true });
  const staging = await mkdtemp(path.join(resourceParent, ".activitywatch-stage-"));
  try {
    await extractWatchers(sourceArchive, staging);
    const extractedRoot = path.join(staging, "activitywatch");
    await validateInstall(extractedRoot, options.target);
    const manifest = await buildManifest(extractedRoot, {
      archive: release.archive,
      sha256: release.sha256,
      source: url,
      target: options.target,
    });
    await writeFile(path.join(extractedRoot, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

    const replacement = path.join(resourceParent, `.activitywatch-ready-${process.pid}`);
    await rm(replacement, { recursive: true, force: true });
    await rename(extractedRoot, replacement);
    await rm(options.output, { recursive: true, force: true });
    await rename(replacement, options.output);
  } catch (error) {
    await rm(path.join(resourceParent, `.activitywatch-ready-${process.pid}`), { recursive: true, force: true });
    throw error;
  } finally {
    await rm(staging, { recursive: true, force: true });
  }

  return { action: "prepared", target: options.target, output: options.output, sha256: release.sha256 };
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    printHelp();
    return;
  }
  const result = await prepare(options);
  console.log(JSON.stringify(result, null, 2));
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : undefined;
if (invokedPath === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`prepare-activitywatch: ${error.message}`);
    process.exitCode = 1;
  });
}
