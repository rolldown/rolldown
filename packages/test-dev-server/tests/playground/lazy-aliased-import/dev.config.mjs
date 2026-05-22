import path from 'node:path';
import url from 'node:url';
import { defineDevConfig } from '@rolldown/test-dev-server';
import { viteAliasPlugin } from 'rolldown/experimental';

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));

// Regression for vitejs/vite#22454. With `experimental.devMode.lazy: true` and
// `viteAliasPlugin`, a dynamic import that uses the alias used to produce a
// proxy module whose id carried `?rolldown-lazy=1?rolldown-lazy=1` (suffix
// appended twice). The doubled key then broke
// `delete __rolldown_runtime__.modules[$STABLE_PROXY_MODULE_ID]` in the proxy
// template (the template substitutes a SINGLE-suffix key), the dedup gate
// skipped the fetched-template re-execution, the real module never registered
// its named exports, and `import('@lazy')` resolved to `{}`.
//
// The cause: `viteAliasPlugin` rewrites `@lazy` -> absolute path and re-enters
// the plugin pipeline via `ctx.resolve(...)`. `skip_self` only excludes
// `viteAliasPlugin` itself from that nested run, so the lazy-compilation
// plugin's `resolve_id` fires twice for the same user import. Each invocation
// appends the marker. The fix in `lazy_compilation_plugin.rs:resolve_id` makes
// the marker append idempotent.
export default defineDevConfig({
  platform: 'browser',
  dev: {
    port: 3640,
  },
  build: {
    input: { main: 'app.js' },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: { lazy: true },
    },
    plugins: [
      viteAliasPlugin({
        entries: [{ find: '@lazy', replacement: path.resolve(__dirname, 'lazy.js') }],
      }),
    ],
  },
});
