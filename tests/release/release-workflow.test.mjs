import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(new URL("../../.github/workflows/release.yml", import.meta.url), "utf8");

test("manual release is a fail-closed beta.2 backfill", () => {
  assert.match(workflow, /type: choice\s+options:\s+- v0\.1\.0-beta\.2/);
  assert.match(workflow, /\[\[ "\$TRIGGER_REF" == "refs\/heads\/main" \]\]/);
  assert.match(workflow, /\[\[ "\$REQUESTED_TAG" == "\$BACKFILL_TAG" \]\]/);
  assert.match(workflow, /BACKFILL_SOURCE_COMMIT: fceb6c7929e0cf469de9af482c0fb21da34062d9/);
  assert.match(workflow, /\[\[ "\$source_commit" == "\$BACKFILL_SOURCE_COMMIT" \]\]/);
});

test("one resolved source SHA feeds and is verified by every job", () => {
  assert.match(workflow, /source_commit: \$\{\{ steps\.resolve\.outputs\.source_commit \}\}/);
  assert.match(workflow, /build:\s+name: Build \$\{\{ matrix\.label \}\}\s+needs: resolve/);
  assert.match(workflow, /release:\s+name: Publish GitHub prerelease\s+needs: \[resolve, build\]/);

  const sourceCheckouts = workflow.match(/ref: \$\{\{ (?:steps\.resolve|needs\.resolve)\.outputs\.source_commit \}\}/g) ?? [];
  assert.equal(sourceCheckouts.length, 3);

  const headChecks = workflow.match(/test "\$\(git rev-parse HEAD\)" = "\$SOURCE_COMMIT"/g) ?? [];
  assert.equal(headChecks.length, 3);

  const finalTagCheck = workflow.indexOf("Verify tag still points to the immutable source");
  const publish = workflow.indexOf("Publish installers and checksums");
  assert.ok(finalTagCheck > 0 && finalTagCheck < publish);
  assert.match(workflow.slice(finalTagCheck, publish), /test "\$current_commit" = "\$SOURCE_COMMIT"/);
});

test("standard and custom attestations cover the same installer manifest", () => {
  const attestUses = workflow.match(/uses: actions\/attest@[0-9a-f]{40}/g) ?? [];
  assert.equal(attestUses.length, 2);

  const provenanceStart = workflow.indexOf("Generate GitHub build provenance");
  const bindingStart = workflow.indexOf("Generate signed release/source binding");
  const stageStart = workflow.indexOf("Stage provenance records");
  assert.ok(provenanceStart > 0 && provenanceStart < bindingStart && bindingStart < stageStart);
  assert.match(workflow.slice(provenanceStart, bindingStart), /subject-checksums: SHA256SUMS/);
  assert.match(workflow.slice(bindingStart, stageStart), /subject-checksums: SHA256SUMS/);
  assert.match(workflow.slice(bindingStart, stageStart), /predicate-path: RELEASE_PROVENANCE\.json/);
  assert.match(workflow, /releaseTag: \$releaseTag/);
  assert.match(workflow, /sourceCommit: \$sourceCommit/);
  assert.match(workflow, /commit: \$workflowCommit/);
  assert.match(workflow, /ref: \$workflowRef/);
  assert.match(workflow, /assets: \$assets/);
});

test("Windows uses a real VBScript smoke test without mutating optional features", () => {
  assert.match(workflow, /os: windows-2022/);
  assert.match(workflow, /Get-Command cscript\.exe/);
  assert.match(workflow, /& \$cscript\.Source \/\/B \/\/NoLogo \$probe/);
  assert.doesNotMatch(workflow, /WindowsOptionalFeature|\bDISM\b/i);
  assert.doesNotMatch(workflow, /windows-2025/);
});

test("every external action is pinned to a full commit SHA", () => {
  const uses = [...workflow.matchAll(/^\s*uses:\s*([^\s#]+)/gm)].map((match) => match[1]);
  assert.ok(uses.length > 0);
  for (const action of uses) {
    if (action.startsWith("./") || action.startsWith("docker://")) continue;
    assert.match(action, /^[^@]+@[0-9a-f]{40}$/, `${action} is not pinned to a full SHA`);
  }
});
