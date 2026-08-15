import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// Node is the default because that is what most of this suite assumes: these
// tests read files and call pure functions. A DOM costs a second per file and
// changes what `import.meta.url` resolves to, which breaks reading a file
// beside the test.
//
// Components ask for a DOM with `// @vitest-environment happy-dom` at the top,
// so the cost lands only where it buys something — and it buys a lot, since an
// interaction tested here is one nobody has to click through again.
export default defineConfig({
  plugins: [svelte()],
  // Svelte ships a browser build and a server build. The component tests drive
  // a DOM, so they need the first; the default would hand them the second and
  // fail on the first piece of state.
  resolve: { conditions: ["browser"] },
  test: {
    environment: "node",
    include: ["tests/**/*.test.ts"],
  },
});
