import { defineDevConfig } from '@rolldown/test-dev-server';

// Reproduction for the lazy-compilation export-name bug fixed in PR #9132.
// Mirrors `src2` of /Users/shuyuan/Examples/lazy-trace: app.js dynamically imports
// page-a, page-b, and selectors. page-a/page-b each statically import selectors,
// so selectors lands in a `ChunkKind::Common` chunk where chunk-level export keys
// get aliased. The fetched proxy must use `loadExports` (runtime registry) instead
// of returning the raw chunk namespace, otherwise direct `import('./selectors')`
// would yield `sel.foo === undefined`.
export default defineDevConfig({
  platform: 'browser',
  dev: {
    port: 3638,
  },
  build: {
    input: { main: 'app.js' },
    output: {
      strictExecutionOrder: true,
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: { lazy: true },
      incrementalBuild: true,
    },
  },
});
