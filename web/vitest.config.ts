import { defineConfig } from "vitest/config";

// Node is the default because that is what the suite already assumes: these
// tests read files and call pure functions. A DOM costs a second per file and
// changes what `import.meta.url` resolves to, which breaks reading a file
// beside the test.
//
// The playground is a page, though, and a good part of what can break is what
// the page does — a row that will not commit, a drop that lands in the wrong
// folder. Those files ask for a DOM with `// @vitest-environment happy-dom` at
// the top, so the cost lands only where it buys something.
export default defineConfig({
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
  },
});
