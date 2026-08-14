# `normfix uninstall`

Removes this binary, and — only when asked by name — the data it created.

```sh
normfix uninstall --dry-run   # show the plan, remove nothing
normfix uninstall             # remove the binary, keep your data
normfix uninstall --purge     # also remove configuration, cache, and backups
```

Installed with Homebrew? Use `brew`, which owns that copy:

```sh
brew uninstall viniciusnevescosta/normfix/normfix
```

`normfix uninstall` refuses a Homebrew-managed binary and prints that command
rather than deleting a file the formula still describes. Your configuration,
cache, and backups live outside the formula, so remove them separately if you
want them gone:

```sh
rm -rf ~/.config/normfix ~/.cache/normfix ~/.local/share/normfix
```

That last path holds the backups and quarantined files, which are the only copy
of anything a previous run replaced or moved.

## It shows the plan first

Nothing is removed before you have seen exactly what would be:

```console
$ normfix uninstall --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  keep    /home/student/.config/normfix (configuration)
  keep    /home/student/.cache/normfix (cache)
  keep    /home/student/.local/share/normfix (backups and quarantine)
Pass --purge to remove the kept directories as well.
```

The default keeps your data. That is deliberate: the backup directory holds the
only copy of anything a previous run replaced or moved, and uninstalling a
formatter is not a statement about wanting to lose the work it saved for you.

## `--purge`

```console
$ normfix uninstall --purge --dry-run
normfix uninstall
  remove  /usr/local/bin/normfix
  remove  /home/student/.config/normfix (configuration)
  remove  /home/student/.cache/normfix (cache)
  remove  /home/student/.local/share/normfix (backups and quarantine)
This also deletes backups and quarantined files, which is the only copy of anything a previous run replaced or moved.
```

Configuration and cache are reproducible: the first is your 42 identity, which
you can supply again, and the second is a cache. Backups and quarantined files
are not. Run [`normfix undo --list`](/commands/undo) first if you are not sure
whether something is still recoverable.

## Confirmation

An interactive run asks before removing anything:

```console
Remove the files listed above? [y/N]
```

`y` is the accepted answer in every language. A non-interactive run — a script,
CI, or `--format json` — refuses instead of assuming, and requires `--force`:

```sh
normfix uninstall --force
normfix uninstall --purge --force
```

## When it refuses

| Situation | What it says |
|---|---|
| Installed by Homebrew | Points you at `brew uninstall viniciusnevescosta/normfix/normfix` |
| No write permission | Names the path and says to check ownership; it never asks for `sudo` |
| A data directory cannot be removed | Names that directory and stops with the binary still installed |

Homebrew is refused rather than worked around: removing a file the formula still
describes leaves `brew` as the only thing that can put the machine back in a
consistent state.

Data directories are removed before the binary. If one of them fails, the tool
that reported the failure is still on disk to retry.

## Removing a binary that is running

On Unix, unlinking the running executable is safe: the kernel keeps the file
alive until the process exits, so the command finishes and prints its result
normally. What is removed is the name in the filesystem.
## Reading it from a script

Every field this command returns is documented in [the JSON API](/reference/api).
