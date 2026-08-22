# Contributing

Bug reports, feature requests, reproduction steps, and product feedback are welcome
through GitHub Issues. Do not include private timeline data, window titles, client
names, credentials, or other sensitive information in an issue.

## Code contributions and relicensing

Ensō Time is source-available and the project may offer separate commercial licenses.
For that reason, external code contributions cannot be merged unless the contributor
has first executed a contributor license agreement (CLA) that expressly grants the
project the rights needed to relicense the contribution, including under commercial
terms.

There is currently no click-through or implied CLA. Opening a pull request, adding a
`Signed-off-by` line, or publishing code under PolyForm does not satisfy this
requirement. Maintainers must not merge an external code contribution until a written
relicensing CLA has been executed and recorded. A pull request submitted without one
may be used for discussion but will be closed or independently reimplemented without
copying the contributed expression.

Please open an issue before investing in a code change. Maintainers can then confirm
whether a CLA process is available and whether the proposed change fits the roadmap.

## Third-party code

Do not submit code, assets, fonts, generated output, or copied snippets unless you have
the right to do so and disclose their exact source and license. ActivityWatch-derived
changes must remain clearly scoped under `vendor/activitywatch/**` and MPL-2.0. Never
remove an upstream copyright, license, or notice.

## Security reports

Do not report vulnerabilities or sensitive diagnostics in a public issue. Follow
[`SECURITY.md`](SECURITY.md) instead.
