import { defineDevConfig } from '@rolldown/test-dev-server';

// Regression test for the lazy-chunk-nested-dynamic-import bug fixed in
// `crates/rolldown/src/hmr/hmr_ast_finalizer.rs::try_rewrite_dynamic_import`
// (the `?rolldown-lazy=1` branch).
//
// Scenario: `outer.js` is itself loaded as a lazy chunk, and its body contains
// `await import('./inner.js')` which also resolves to a lazy proxy. The dynamic
// import inside the lazy chunk's HMR partial bundle must be rewritten to:
//
//   import('/@vite/lazy?id=...').then(() => __rolldown_runtime__.loadExports("<stable_proxy_id>"))
//
// so that `__unwrap_lazy_compilation_entry` finds `'rolldown:exports'` on the
// registered proxy module instead of the raw partial-bundle namespace.
export default defineDevConfig({
  platform: 'browser',
  dev: {
    port: 3639,
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
