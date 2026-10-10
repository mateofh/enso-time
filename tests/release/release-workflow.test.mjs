import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

// Windows checkouts may convert to CRLF; the job-splitting assertions expect LF.
const workflow = readFileSync(new URL("../../.github/workflows/release.yml", import.meta.url), "utf8").replace(/\r\n/g, "\n");
const provenanceDocs = readFileSync(new URL("../../docs/RELEASE_PROVENANCE.md", import.meta.url), "utf8");
const tauriConfig = JSON.parse(readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf8"));

test("manual release is a fail-closed beta.2 backfill", () => {
  assert.match(workflow, /type: choice\s+options:\s+- v0\.1\.0-beta\.2/);
  assert.match(workflow, /\[\[ "\$TRIGGER_REF" == "refs\/heads\/main" \]\]/);
  assert.match(workflow, /\[\[ "\$REQUESTED_TAG" == "\$BACKFILL_TAG" \]\]/);
  assert.match(workflow, /BACKFILL_SOURCE_COMMIT: fceb6c7929e0cf469de9af482c0fb21da34062d9/);
  assert.match(workflow, /\[\[ "\$source_commit" == "\$BACKFILL_SOURCE_COMMIT" \]\]/);
  assert.match(workflow, /\[\[ "\$TRIGGER_SHA" == "\$WORKFLOW_COMMIT" \]\]/);
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
  assert.match(workflow, /\[\[ "\$WORKFLOW_COMMIT" == "\$source_commit" \]\]/);
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

test("only the publisher crosses the protected release environment", () => {
  const buildStart = workflow.indexOf("\n  build:");
  const releaseStart = workflow.indexOf("\n  release:");
  assert.ok(buildStart > 0 && buildStart < releaseStart);
  assert.doesNotMatch(workflow.slice(buildStart, releaseStart), /environment:\s*release/);
  assert.match(workflow.slice(releaseStart), /runs-on: ubuntu-24\.04\s+environment: release\s+timeout-minutes:/);
  assert.equal(workflow.match(/environment:\s*release/g)?.length, 1);
  assert.match(workflow, /group: release-\$\{\{ inputs\.release_tag \|\| github\.ref_name \}\}/);
  assert.match(workflow, /cancel-in-progress: false/);
});

test("verification pins signer and OIDC source digests for both trigger branches", () => {
  assert.match(workflow, /--signer-digest \$\{\{ needs\.resolve\.outputs\.workflow_commit \}\}/);
  assert.match(workflow, /--source-digest \$\{\{ github\.sha \}\}/);

  assert.match(provenanceDocs, /Future tag push: `WORKFLOW_COMMIT=SOURCE_COMMIT=OIDC_SOURCE_COMMIT=<peeled tag commit>`/);
  assert.match(provenanceDocs, /Beta\.2 backfill: `OIDC_SOURCE_COMMIT=WORKFLOW_COMMIT=<audited main commit>`/);
  assert.match(provenanceDocs, /`SOURCE_COMMIT=fceb6c7929e0cf469de9af482c0fb21da34062d9`/);
  assert.match(provenanceDocs, /--signer-digest "\$WORKFLOW_COMMIT"/);
  assert.match(provenanceDocs, /--source-digest "\$OIDC_SOURCE_COMMIT"/);
  assert.match(provenanceDocs, /\[\[ "\$commit" =~ \^\[0-9a-f\]\{40\}\$ \]\]/);
  assert.match(provenanceDocs, /--predicate-type "\$PREDICATE_TYPE"/);
  assert.match(provenanceDocs, /any\(\.subject\[\]; \.name == \$name and \.digest\.sha256 == \$sha256\)/);
  assert.match(provenanceDocs, /\.predicate\.releaseTag == \$releaseTag/);
  assert.match(provenanceDocs, /any\(\.predicate\.assets\[\]; \.name == \$name and \.sha256 == \$sha256\)/);
});

test("Windows uses a real VBScript smoke test without mutating optional features", () => {
  assert.match(workflow, /os: windows-2022/);
  assert.match(workflow, /Get-Command cscript\.exe/);
  assert.match(workflow, /& \$cscript\.Source \/\/B \/\/NoLogo \$probe/);
  assert.doesNotMatch(workflow, /WindowsOptionalFeature|\bDISM\b/i);
  assert.doesNotMatch(workflow, /windows-2025/);
});

test("Windows packaging uses an ASCII product name without changing the macOS brand", () => {
  const configureStart = workflow.indexOf("- name: Configure release packaging");
  const buildStart = workflow.indexOf("- name: Build Tauri installers", configureStart);
  assert.ok(configureStart > 0 && buildStart > configureStart);
  const configure = workflow.slice(configureStart, buildStart);
  const windowsStart = configure.indexOf('if (process.platform === "win32")');
  assert.ok(windowsStart > 0);
  const windows = configure.slice(windowsStart);
  const windowsName = windows.match(/const windowsProductName = "([^"]+)"/)?.[1];
  const windowsUpgradeCode = windows.match(/const windowsUpgradeCode = "([^"]+)"/)?.[1];

  assert.equal(windowsName, "Enso Time");
  assert.equal(windowsUpgradeCode, "b9b13e4b-5399-58c4-b563-de7a5e0c02a9");
  assert.match(windowsName, /^[\x20-\x7E]+$/);
  assert.match(windows, /config\.productName = windowsProductName/);
  assert.match(windows, /config\.bundle = \{ windows: \{ wix: \{ upgradeCode: windowsUpgradeCode \} \} \}/);
  assert.match(windows, /Windows productName must be printable ASCII/);

  // macOS keeps the configured brand: only the Windows branch renames it.
  assert.doesNotMatch(configure.slice(0, windowsStart), /productName/);
  assert.doesNotMatch(configure, /signingIdentity: "-"/);
  assert.equal(tauriConfig.productName, "Ensō Time");
});

test("Apple signing secrets are reachable only through the protected macOS environment", () => {
  const buildStart = workflow.indexOf("\n  build:");
  const nextJob = workflow.slice(buildStart + 1).search(/\n  [a-z][a-z-]*:\n/) + buildStart + 1;
  const build = workflow.slice(buildStart, nextJob);
  assert.match(build, /environment: \$\{\{ matrix\.signing_environment \}\}/);
  assert.equal(build.match(/signing_environment: macos-signing/g)?.length, 2);
  assert.match(build, /os: windows-2022[\s\S]*?signing_environment: ''/);
  const secretJobs = workflow.split(/\n  (?=[a-z][a-z-]*:\n)/).filter((job) => /secrets\.APPLE_/.test(job));
  assert.equal(secretJobs.length, 1);
  assert.ok(secretJobs[0].startsWith("build:"));
  assert.match(build, /if: always\(\) && runner\.os == 'macOS'[\s\S]*?security delete-keychain/);
});

test("Windows verifies MSI identity through COM and enables verbose Tauri diagnostics", () => {
  assert.match(workflow, /if: runner\.os == 'Windows'\s+shell: bash\s+run: echo 'TAURI_CLI_VERBOSITY=1' >> "\$GITHUB_ENV"/);
  assert.match(workflow, /New-Object -ComObject WindowsInstaller\.Installer/);
  assert.match(workflow, /Get-MsiProperty -Database \$database -Name "ProductName"/);
  assert.match(workflow, /Get-MsiProperty -Database \$database -Name "UpgradeCode"/);
  assert.match(workflow, /if \(\$productName -cne "Enso Time"\)/);
  assert.match(workflow, /if \(\$upgradeCode -cne "b9b13e4b-5399-58c4-b563-de7a5e0c02a9"\)/);
  assert.match(workflow, /FinalReleaseComObject\(\$record\)/);
  assert.match(workflow, /FinalReleaseComObject\(\$view\)/);
  assert.match(workflow, /FinalReleaseComObject\(\$database\)/);
  assert.match(workflow, /FinalReleaseComObject\(\$installer\)/);
});

test("every external action is pinned to a full commit SHA", () => {
  const uses = [...workflow.matchAll(/^\s*uses:\s*([^\s#]+)/gm)].map((match) => match[1]);
  assert.ok(uses.length > 0);
  for (const action of uses) {
    if (action.startsWith("./") || action.startsWith("docker://")) continue;
    assert.match(action, /^[^@]+@[0-9a-f]{40}$/, `${action} is not pinned to a full SHA`);
  }
});
