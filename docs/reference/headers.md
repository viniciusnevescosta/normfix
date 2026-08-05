# Official 42 headers

How the header block, the identity behind it, and header inclusion guards are handled.

Missing official headers are inserted into C sources, C headers, and Makefiles
when a validated identity is available. Identity resolution uses this order:

1. `--email`, with optional `--login` consistency checking;
2. `NORMFIX_EMAIL`, with an optional environment or CLI login;
3. an INI configuration file;
4. the effective Git `user.email`, if it is a supported 42 address;
5. the `MAIL` environment variable;
6. known Vim, Neovim, VS Code, Cursor, and VSCodium 42-header settings.

The email is the source of truth. The login is the local part before `@`; the
tool never invents an address or silently chooses between ambiguous saved
addresses.

When no valid email is found and both input and error output are interactive
terminals, human mode asks:

```text
No verified 42 student email was found.
Enter your 42 email (Enter, cancel, or q to skip the header):
```

Enter, `cancel`, `q`, or end-of-input skips header insertion while all other
safe fixes continue. JSON and non-interactive runs never prompt. Ctrl-C cancels
the command itself, following normal terminal behavior.

### Persistent identity configuration

The default configuration path is:

```text
$XDG_CONFIG_HOME/normfix/config.ini
```

When `XDG_CONFIG_HOME` is not set, it falls back to:

```text
~/.config/normfix/config.ini
```

Use `NORMFIX_CONFIG` to select another file. The supported format is:

```ini
[header]
login = your_login
email = your_login@student.42.fr
```

Environment configuration is also supported:

```sh
export NORMFIX_LOGIN='your_login'
export NORMFIX_EMAIL='your_login@student.42.fr'
```

One timestamp is captured for the complete run. `SOURCE_DATE_EPOCH` can provide
a reproducible UTC timestamp; an invalid value stops the run instead of
silently using the wall clock.

Valid existing headers retain the `By` and `Created` fields. The filename and
`Updated` line change only when the file has another accepted edit or its header
filename is stale, making a second clean run idempotent.

### Header guards

For ordinary headers, `normfix` can insert a missing filename-derived guard,
repair a mismatched `#ifndef`/`#define` pair, or rename a simple wrong guard.
Every operation requires a closed Git-worktree proof. The proof scans ignored
files too, verifies the expected macro is unused, rejects duplicate
filename-derived guards and dynamic build definitions, and binds approval to
the complete project and header hashes.

Insertion is refused for conditional preprocessing, `#pragma once`, `#undef`,
or another macro collision. A rename is refused when the old names have uses
beyond the canonical whole-file pair. Complex, referenced, repeated-inclusion,
non-Git, or ambiguous headers stay unchanged and receive an actionable warning.
