#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";
import process from "node:process";
import { TARGETS } from "./prepare-activitywatch.mjs";

const root = resolve(import.meta.dirname, "..");
const strictManifests = process.argv.includes("--strict-manifests");
const simulateCrlf = process.argv.includes("--simulate-crlf");
const errors = [];
const warnings = [];

const expected = {
  version: "0.2.0-beta.1",
  licenseId: "PolyForm-Noncommercial-1.0.0",
  polyformSha256: "c72b8a4b508c2363a360d29530e17767dd0e44c513b253e6ac5ab720876cd426",
  mplSha256: "1f256ecad192880510e84ad60474eab7589218784b9a50bc7ceee34c2b91f1d5",
  requiredNotice: "Required Notice: Copyright 2026 Mateo (@mateofh).",
};

function normalizeLineEndings(value) {
  return Buffer.from(value.toString("latin1").replaceAll("\r\n", "\n"), "latin1");
}

function convertToCrlf(value) {
  const normalized = normalizeLineEndings(value);
  return Buffer.from(normalized.toString("latin1").replaceAll("\n", "\r\n"), "latin1");
}

async function bytes(path) {
  const value = await readFile(resolve(root, path));
  return simulateCrlf ? convertToCrlf(value) : value;
}

async function text(path) {
  return normalizeLineEndings(await bytes(path)).toString("utf8");
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function requireCondition(condition, message) {
  if (!condition) errors.push(message);
}

async function requireIdentical(left, right) {
  const [leftBytes, rightBytes] = await Promise.all([bytes(left), bytes(right)]);
  requireCondition(
    normalizeLineEndings(leftBytes).equals(normalizeLineEndings(rightBytes)),
    `${right} must match ${left}, allowing only CRLF/LF representation differences`,
  );
}

const polyform = await bytes("LICENSE");
const mpl = await bytes("LICENSES/MPL-2.0.txt");
requireCondition(
  sha256(normalizeLineEndings(polyform)) === expected.polyformSha256,
  "LICENSE differs from the canonical PolyForm Noncommercial 1.0.0 text plus the required notice",
);
requireCondition(
  sha256(normalizeLineEndings(mpl)) === expected.mplSha256,
  "LICENSES/MPL-2.0.txt differs from the canonical MPL-2.0 text",
);

const polyformText = normalizeLineEndings(polyform).toString("utf8");
requireCondition(
  polyformText.split(expected.requiredNotice).length === 2,
  "LICENSE must contain the exact Required Notice once",
);

for (const [source, packaged] of [
  ["LICENSE", "src-tauri/resources/legal/POLYFORM-NONCOMMERCIAL-1.0.0.txt"],
  ["LICENSES/MPL-2.0.txt", "src-tauri/resources/legal/MPL-2.0.txt"],
  ["COMMERCIAL.md", "src-tauri/resources/legal/COMMERCIAL.md"],
  ["TRADEMARKS.md", "src-tauri/resources/legal/TRADEMARKS.md"],
  ["PRIVACY.md", "src-tauri/resources/legal/PRIVACY.md"],
  ["SECURITY.md", "src-tauri/resources/legal/SECURITY.md"],
  ["THIRD_PARTY_NOTICES.md", "src-tauri/resources/legal/THIRD_PARTY_NOTICES.md"],
]) {
  await requireIdentical(source, packaged);
}

const licenseMap = await text("LICENSES/README.md");
const thirdParty = await text("THIRD_PARTY_NOTICES.md");
const trademarks = await text("TRADEMARKS.md");
const commercial = await text("COMMERCIAL.md");
for (const phrase of ["vendor/activitywatch/**", "Mozilla Public License 2.0", "All rights reserved"]) {
  requireCondition(licenseMap.includes(phrase), `license map is missing boundary phrase: ${phrase}`);
}
for (const phrase of ["vendor/activitywatch/**", "remain covered by MPL-2.0", "does not apply"]) {
  requireCondition(thirdParty.includes(phrase), `third-party notice is missing boundary phrase: ${phrase}`);
}
for (const phrase of ["isotype", "app icons", "do not grant permission"]) {
  requireCondition(trademarks.includes(phrase), `trademark policy is missing: ${phrase}`);
}
for (const phrase of [
  "internal use by a for-profit company",
  "resale",
  "paid consulting",
  "bundling",
  "providing services commercially",
  "separate written agreement",
  "@mateofh",
]) {
  requireCondition(commercial.includes(phrase), `commercial policy is missing: ${phrase}`);
}

const vendorFiles = await readdir(resolve(root, "vendor/activitywatch"), { recursive: true });
requireCondition(vendorFiles.length > 20, "vendor/activitywatch does not contain the expected source tree");
const upstreamCommit = (await text("vendor/activitywatch/UPSTREAM_COMMIT")).trim();
requireCondition(/^[0-9a-f]{40}$/.test(upstreamCommit), "ActivityWatch UPSTREAM_COMMIT must be a full commit SHA");
const vendorReadme = await text("vendor/activitywatch/README.md");
requireCondition(vendorReadme.includes("Mozilla Public License 2.0"), "ActivityWatch README must identify MPL-2.0");
if (vendorReadme.includes("repository root `LICENSE`")) {
  warnings.push(
    "handoff: vendor/activitywatch/README.md still points to the root LICENSE; its owner must point it to LICENSES/MPL-2.0.txt",
  );
}

const noticeHashRows = [...thirdParty.matchAll(/^\| `([^`]+)` \| `([0-9a-f]{64})` \|$/gm)];
const noticeHashes = new Map(noticeHashRows.map((match) => [match[1], match[2]]));
requireCondition(
  noticeHashRows.length === Object.keys(TARGETS).length
    && noticeHashes.size === Object.keys(TARGETS).length,
  "third-party notice must contain exactly one checksum row for every ActivityWatch target",
);
for (const [target, metadata] of Object.entries(TARGETS)) {
  requireCondition(
    noticeHashes.get(target) === metadata.sha256,
    `ActivityWatch checksum mismatch for ${target}: preparer=${metadata.sha256}, notice=${noticeHashes.get(target) ?? "missing"}`,
  );
}

const packageJson = JSON.parse(await text("package.json"));
const tauriConfig = JSON.parse(await text("src-tauri/tauri.conf.json"));
const cargoToml = await text("src-tauri/Cargo.toml");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const cargoLicense = cargoToml.match(/^license\s*=\s*"([^"]+)"/m)?.[1];
const manifestProblems = [];

for (const [name, value] of [
  ["package.json", packageJson.version],
  ["src-tauri/Cargo.toml", cargoVersion],
  ["src-tauri/tauri.conf.json", tauriConfig.version],
]) {
  if (value !== expected.version) manifestProblems.push(`${name} version is ${value}; expected ${expected.version}`);
}
for (const [name, value] of [
  ["package.json", packageJson.license],
  ["src-tauri/Cargo.toml", cargoLicense],
]) {
  if (value !== expected.licenseId) manifestProblems.push(`${name} license is ${value}; expected ${expected.licenseId}`);
}

requireCondition(
  Array.isArray(tauriConfig.bundle?.resources) && tauriConfig.bundle.resources.includes("resources/**/*"),
  "Tauri bundle must include resources/**/* so packaged legal terms are distributed",
);

const attributes = new Set((await text(".gitattributes")).split("\n"));
for (const rule of [
  "LICENSE text eol=lf",
  "LICENSES/** text eol=lf",
  "COMMERCIAL.md text eol=lf",
  "PRIVACY.md text eol=lf",
  "SECURITY.md text eol=lf",
  "THIRD_PARTY_NOTICES.md text eol=lf",
  "TRADEMARKS.md text eol=lf",
  "src-tauri/resources/legal/** text eol=lf",
]) {
  requireCondition(attributes.has(rule), `.gitattributes is missing required LF policy: ${rule}`);
}

if (manifestProblems.length > 0) {
  const prefix = strictManifests ? "" : "handoff: ";
  (strictManifests ? errors : warnings).push(...manifestProblems.map((problem) => prefix + problem));
}

for (const warning of warnings) console.warn(`warning: ${warning}`);
if (errors.length > 0) {
  for (const error of errors) console.error(`error: ${error}`);
  process.exitCode = 1;
} else {
  const mode = [
    strictManifests ? "strict" : "handoff-aware",
    simulateCrlf ? "simulated-CRLF" : "native-line-endings",
  ].join(", ");
  console.log(`License boundaries and packaged notices validated (${mode} mode).`);
}
