# `normfix upgrade`

Replaces the running binary with the newest published release in its update
channel.

```sh
normfix upgrade          # download, verify, and install
normfix upgrade --check  # report only
```

```console
$ normfix upgrade --check
normfix 1.0.0-rc.1 is already the newest release.
```

## What it does, in order

1. Selects the update channel from the running version. A stable build asks
   GitHub's `/releases/latest` endpoint for the newest stable release. A
   pre-release follows the complete release feed, so it can advance to a newer
   release candidate or to the eventual stable release.
2. Compares Semantic Versioning precedence and stops if the published release
   is equal to or older than the running build. An update never downgrades.
3. Refuses if the binary is managed by Homebrew or Scoop, and tells you the
   package-manager command that keeps the installation consistent.
4. Downloads the archive for your platform and the published `SHA256SUMS`, with
   bounded connection time, total time, and file size.
5. **Verifies the digest.** A missing, duplicate, malformed, or mismatched
   entry aborts before extraction; nothing is written.
6. Accepts only `normfix`, `README.md`, and `LICENSE` in the archive, then
   extracts into a private staging directory *inside* the destination. The final
   step is a rename on the same filesystem: the binary is either replaced or
   left exactly as it was.

Replacing a running executable is safe on Unix, because the running process
keeps the old file until it exits.

The channel boundary is deliberate: a stable installation is never moved to a
beta or release candidate by `upgrade` or by the daily release notice. Opting
into a pre-release remains an explicit install-time choice.

## When it refuses

| Situation | What it says |
|---|---|
| Installed by Homebrew | Points you at `brew upgrade viniciusnevescosta/normfix/normfix` |
| Installed by Scoop | Points you at `scoop update normfix` |
| Direct install on Windows | `--check` works; installation asks you to rerun the verified installer because a running `.exe` cannot replace itself |
| No write permission | Names the path and says to check ownership; it never asks for `sudo` |
| Invalid checksum manifest or mismatch | Names the invalid entry or prints both digests and installs nothing |
| No `curl` or `wget` | Says which tool is missing |
| Unsupported platform | Suggests building from source or using the playground |

## The release notice

A normal run prints one line when a newer release exists:

```text
normfix 1.0.0 is available; this is 1.0.0-rc.1. Run `normfix upgrade`.
```

This is the only network access outside `upgrade` itself, so it is deliberately
narrow:

- at most **once a day**, with the timestamp cached under
  `$XDG_CACHE_HOME/normfix/last-update-check`;
- only for **interactive human output**, never for `--format json` and never
  when stderr is not a terminal, so scripts and CI are unaffected;
- **silent on any failure**, because a formatter that cannot reach the network
  has nothing wrong with it;
- the attempt is recorded *before* the request, so an unreachable network
  cannot make every run pay for the same lookup.

Disable it entirely:

```sh
export NORMFIX_NO_UPDATE_CHECK=1
```

::: tip Nothing about your code leaves the machine
The check asks GitHub for public release metadata. It sends no path, no source,
and no identifier of any kind.
:::
## Reading it from a script

Every field this command returns is documented in [the JSON API](/reference/api).
