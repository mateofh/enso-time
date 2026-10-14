import { readFileSync } from "node:fs";

export const REQUIRED_TARGETS = ["macos-arm64", "macos-x86_64", "windows-x86_64"];
export const REQUIRED_CHECKS = ["cleanInstall", "upgradeFromBeta", "signatures", "captureAndAfk", "permissionsDenied", "pauseResume", "suspendRestart", "autostart", "fullBlueprintJourney", "exportRetentionDeletion", "localPrivacy", "languagesKeyboardContrast"];

export function validateStableQa(evidence, sourceCommit, checksums) {
  const require = (condition, message) => { if (!condition) throw new Error(message); };
  require(/^[a-f0-9]{40}$/.test(sourceCommit ?? ""), "A full source commit is required");
  require(evidence?.schemaVersion === 1 && evidence.sourceCommit === sourceCommit, "QA evidence must match the exact release source commit");
  for (const name of ["code", "security"]) {
    const review = evidence.reviews?.[name];
    require(review?.status === "passed", `${name} review must pass`);
    for (const severity of ["critical", "high", "medium"]) require(review.unresolved?.[severity] === 0, `${name} review has unresolved ${severity} findings`);
  }
  require(evidence.pilot?.participants >= 5 && evidence.pilot.days >= 7 && evidence.pilot.blockingIssues === 0, "Pilot requires 5 participants, 7 days, and no blocking issues");
  for (const target of REQUIRED_TARGETS) {
    const matches = evidence.platforms?.filter((platform) => platform.target === target) ?? [];
    require(matches.length === 1, `Exactly one platform record is required for ${target}`);
    require(typeof matches[0].osVersion === "string" && matches[0].osVersion.length > 0, `OS version required for ${target}`);
    for (const check of REQUIRED_CHECKS) require(matches[0].checks?.[check] === "passed", `${target}: ${check} must pass`);
  }
  const assets = checksums.trim().split(/\r?\n/).map((line) => {
    const match = /^([a-f0-9]{64}) [ *](.+)$/.exec(line);
    require(Boolean(match), "Malformed SHA256SUMS");
    return { sha256: match[1], name: match[2] };
  });
  require(assets.length === 4 && new Set(assets.map((asset) => asset.name)).size === 4, "Exactly four unique installers are required");
  require(Array.isArray(evidence.artifacts) && evidence.artifacts.length === assets.length, "QA evidence must cover every installer");
  for (const asset of assets) {
    const matches = evidence.artifacts.filter((item) => item.name === asset.name && item.sha256 === asset.sha256);
    require(matches.length === 1, `QA evidence hash mismatch: ${asset.name}`);
  }
  require(typeof evidence.reviewedBy === "string" && evidence.reviewedBy.trim().length > 0, "A human reviewer must be recorded");
}

if (process.argv[1]?.endsWith("stable-qa.mjs")) {
  const evidence = JSON.parse(process.env.STABLE_QA_EVIDENCE || "null");
  validateStableQa(evidence, process.env.SOURCE_COMMIT, readFileSync("SHA256SUMS", "utf8"));
  console.log("Stable QA evidence matches source, installer hashes, reviews, platforms, and pilot.");
}
