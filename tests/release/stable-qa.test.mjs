import test from "node:test";
import assert from "node:assert/strict";
import { validateStableQa, REQUIRED_CHECKS, REQUIRED_TARGETS } from "../../scripts/stable-qa.mjs";

const source = "a".repeat(40);
const artifacts = ["macos-arm64.dmg", "macos-x86_64.dmg", "windows.msi", "windows.exe"].map((name, index) => ({ name, sha256: String(index).repeat(64) }));
const checksums = artifacts.map((item) => `${item.sha256}  ${item.name}`).join("\n");
function evidence() {
  const review = { status: "passed", unresolved: { critical: 0, high: 0, medium: 0 } };
  return { schemaVersion: 1, sourceCommit: source, reviewedBy: "Release owner", artifacts,
    reviews: { code: structuredClone(review), security: structuredClone(review) },
    pilot: { participants: 5, days: 7, blockingIssues: 0 },
    platforms: REQUIRED_TARGETS.map((target) => ({ target, osVersion: "Test OS", checks: Object.fromEntries(REQUIRED_CHECKS.map((check) => [check, "passed"])) })) };
}

test("approves only evidence for the exact installers and source", () => {
  assert.doesNotThrow(() => validateStableQa(evidence(), source, checksums));
  assert.throws(() => validateStableQa(evidence(), "b".repeat(40), checksums), /source commit/);
  assert.throws(() => validateStableQa(evidence(), source, checksums.replace("macos-arm64", "changed")), /hash mismatch/);
  assert.throws(() => validateStableQa(null, source, checksums), /source commit/);
});

test("rejects unfinished reviews, platform checks, and pilot", () => {
  const code = evidence(); code.reviews.code.unresolved.medium = 1;
  assert.throws(() => validateStableQa(code, source, checksums), /medium/);
  const pilot = evidence(); pilot.pilot.days = 6;
  assert.throws(() => validateStableQa(pilot, source, checksums), /Pilot/);
  const platform = evidence(); platform.platforms[0].checks.captureAndAfk = "pending";
  assert.throws(() => validateStableQa(platform, source, checksums), /captureAndAfk/);
  const absent = evidence(); absent.platforms.pop();
  assert.throws(() => validateStableQa(absent, source, checksums), /windows-x86_64/);
});
