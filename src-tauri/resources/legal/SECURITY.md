# Security policy

## Supported version

Security fixes are currently provided only for the latest public `0.1.0-beta.*`
release. This beta is provided without a warranty; support for a version can end when a
new beta is published.

## Reporting a vulnerability

Please report suspected vulnerabilities privately using this repository's GitHub
**Security → Report a vulnerability** flow. If private vulnerability reporting is not
enabled, contact [@mateofh](https://github.com/mateofh) and ask for a private reporting
channel without including exploit details in the initial public message.

Do not open a public issue containing a vulnerability, proof of concept, private
timeline data, window titles, local database contents, credentials, or identifying
diagnostics. Include affected versions, operating system, impact, minimal reproduction
steps, and suggested mitigations only through the private channel.

No response time, bounty, embargo, or disclosure date is promised. Please allow a
reasonable opportunity to investigate and prepare a fix before public disclosure.

## Installer trust

The public beta is intentionally distributed without paid code-signing or notarization
certificates. macOS application contents use ad-hoc signing for packaging; Windows MSI
and NSIS installers are unsigned. Gatekeeper or SmartScreen warnings are therefore
expected. Verify the release's `SHA256SUMS` entry against your download and obtain
installers only from the project's GitHub Releases page. A checksum confirms file
identity, not publisher identity or safety.

The release workflow also creates GitHub artifact attestations with a short-lived OIDC
identity and attaches the Sigstore bundle to the release. Use `gh attestation verify`
as documented in the README to check that an installer digest was produced by this
repository's release workflow. This provenance is complementary supply-chain evidence;
it is not Apple Developer ID signing/notarization, Windows Authenticode, malware
analysis, or a warranty, and it does not suppress operating-system warnings.
