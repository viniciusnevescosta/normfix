# `normfix upgrade`

Replaces the running binary with the newest published release.

```sh
normfix upgrade          # download, verify, and install
normfix upgrade --check  # report only
```

```console
$ normfix upgrade --check
normfix 0.4.0-beta.3 is already the newest release.
```

## What it does, in order

1. Asks GitHub for the newest release tag. The releases listing is used rather
   than `/releases/latest`, which answers 404 while every published version is
   a pre-release.
2. Stops if you already run it.
3. Refuses if the binary is managed by Homebrew, and tells you the command that
   does the right thing there.
4. Downloads the archive for your platform and the published `SHA256SUMS`.
5. **Verifies the digest.** A mismatch aborts and prints both values; nothing
   is written.
6. Extracts into a staging directory *inside* the destination, so the final
   step is a rename on the same filesystem: the binary is either replaced or
   left exactly as it was.

Replacing a running executable is safe on Unix, because the running process
keeps the old file until it exits.

## When it refuses

| Situation | What it says |
|---|---|
| Installed by Homebrew | Points you at `brew upgrade viniciusnevescosta/normfix/normfix` |
| No write permission | Names the path and says to check ownership; it never asks for `sudo` |
| Checksum mismatch | Prints both digests and installs nothing |
| No `curl` or `wget` | Says which tool is missing |
| Unsupported platform | Suggests building from source or using the playground |

## The release notice

A normal run prints one line when a newer release exists:

```text
normfix 0.4.0-beta.4 is available; this is 0.4.0-beta.3. Run `normfix upgrade`.
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
The check asks GitHub for a version list. It sends no path, no source, and no
identifier of any kind.
:::
