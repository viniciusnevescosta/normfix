# normfix documentation

This directory holds both the reference documents and the VitePress site that
publishes them.

Every `.md` file here is written to be read two ways: directly on GitHub, and
rendered as a page of the documentation site. Keep links working in both
contexts — a repository-relative link that leaves this directory has to be
exempted in `.vitepress/config.ts`, so prefer linking within `docs/`.

## Layout

| Path | What it is |
|---|---|
| `index.md` | Site home; not shown as a page in the sidebar |
| `guide/` | Task-oriented pages: getting started, command line, playground |
| `ARCHITECTURE.md` | What each crate owns, why the boundaries exist, and the invariants |
| `COMPATIBILITY.md` | The exact supported Norminette, MSRV, and release targets |
| `RELEASING.md` | How a tagged release is produced and verified |
| `.vitepress/` | Site configuration and the theme extension |

Release notes live in [`../CHANGELOG.md`](../CHANGELOG.md), outside this
directory, so they stay next to the code they describe.

## Running the site

The playground and this documentation are one npm workspace rooted at the
repository, so install once from there:

```sh
npm ci
npm run docs:dev
```

`npm run build` from the repository root builds the playground and then this
documentation into it. Run `npm run dev` for the playground instead.

The site is published beneath the browser playground: the playground owns the
site root and this documentation is served from `/docs/`, which is why
`.vitepress/config.ts` sets `base` to `/docs/` and writes `outDir` into the
playground bundle at `../web/dist/docs`. Building the documentation alone
therefore requires the playground bundle to be built first, or its output
directory will contain only the documentation.

## Conventions

- British or American spelling is fine, but stay consistent within a page.
- Prefer a table over a bulleted list when every item shares the same shape.
- State what the tool refuses to do as plainly as what it does; the refusals
  are the product.
- A `mermaid` fenced block renders as a diagram on the site and as source on
  GitHub. Keep diagrams small enough to stay readable in both.
- When a page documents a flag, the flag must also appear in the README's
  option table. The two are checked by hand, not by a test.
