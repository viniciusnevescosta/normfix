# Security policy

`normfix` edits source files in place and runs external tools. That makes two
classes of problem security-relevant, not merely bugs:

- anything that lets a repository under analysis cause a write outside the
  project, escape a symbolic-link or path check, or execute an unexpected
  program;
- anything that makes a published release archive differ from the tagged
  source it claims to be built from.

## Supported versions

Only the latest published release receives fixes. Pre-release versions are
supported until the next pre-release replaces them.

## Reporting

Report privately through
[GitHub security advisories](https://github.com/viniciusnevescosta/normfix/security/advisories/new).
Please do not open a public issue for an unfixed vulnerability.

Include the version (`normfix --version`), the operating system, and the
smallest project layout that reproduces the problem. A failing path check is
usually reproducible with two directories and a symbolic link.

You should get an acknowledgement within a week. If a fix is warranted, the
advisory is published together with the release that carries it.

## What is not a vulnerability

- **A formatting result you disagree with.** Open an issue instead.
- **A diagnostic that the official Norminette does not report**, or the
  reverse. Compatibility differences are ordinary bugs; the official checker
  remains the authority.
- **A destructive operation you authorized.** `--unsafe`, `--force`,
  `--remove-unused`, `--remove-unexpected`, and `--remove-invalid-comments`
  delete or move files on purpose, and every one of them keeps recoverable
  external storage. Losing a file you explicitly told the tool to remove, and
  then discarding the backup, is not a vulnerability.
- **Compiler or analyzer findings.** Those come from `cc`, are advisory, and
  never authorize an edit.

## Verifying a release

Every archive is published with a `SHA256SUMS` manifest and GitHub build
provenance attestation:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
grep " normfix-aarch64-macos.tar.gz$" SHA256SUMS | shasum -a 256 -c -
```

A binary that fails either check did not come from this repository's release
workflow.
