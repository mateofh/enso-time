import { readFileSync, writeFileSync } from "node:fs";

export const WINDOWS_UPGRADE_CODE = "b9b13e4b-5399-58c4-b563-de7a5e0c02a9";
export function releaseConfig(version, platform, signing = {}) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:-beta\.(\d+))?$/.exec(version);
  if (!match) throw new Error("Only stable and beta.N release versions are supported");
  const stable = match[4] === undefined;
  if (platform === "darwin") {
    if (!signing.appleIdentity?.startsWith("Developer ID Application:")) throw new Error("Stable macOS releases require a Developer ID signing identity");
    return { bundle: { macOS: { signingIdentity: signing.appleIdentity } } };
  }
  if (platform === "win32") {
    if (stable && !/^[0-9a-f]{40}$/i.test(signing.windowsThumbprint ?? "")) throw new Error("Stable Windows releases require a signing certificate thumbprint");
    const [major, minor, patch] = match.slice(1, 4).map(Number);
    const beta = stable ? 99 : Number(match[4]);
    if (major > 255 || minor > 255 || !Number.isSafeInteger(patch) || patch * 100 + beta > 65535 || (!stable && (beta < 1 || beta > 98))) {
      throw new Error("Version exceeds Windows installer bounds (beta.1 through beta.98)");
    }
    const base = `${major}.${minor}.${patch}`;
    // MSI compares only three fields; NSIS compares four. Reserve a stable
    // ordinal so upgrades from the already published numeric-beta installers
    // and future betas remain ordered, without changing the application's SemVer.
    return {
      productName: "Enso Time",
      version: `${base}-${stable ? 65535 : beta}`,
      bundle: { windows: {
        wix: { upgradeCode: WINDOWS_UPGRADE_CODE, version: `${major}.${minor}.${patch * 100 + beta}` },
        ...(stable ? { certificateThumbprint: signing.windowsThumbprint, digestAlgorithm: "sha256", timestampUrl: "https://timestamp.digicert.com", tsp: true } : {}),
      } },
    };
  }
  throw new Error(`Unsupported release platform: ${platform}`);
}

if (process.argv[1] === new URL(import.meta.url).pathname || process.argv[1]?.endsWith("release-config.mjs")) {
  const { version } = JSON.parse(readFileSync("package.json", "utf8"));
  const config = releaseConfig(version, process.platform, {
    appleIdentity: process.env.APPLE_SIGNING_IDENTITY,
    windowsThumbprint: process.env.WINDOWS_CERTIFICATE_THUMBPRINT,
  });
  writeFileSync("src-tauri/ci-bundle.json", `${JSON.stringify(config, null, 2)}\n`);
}
