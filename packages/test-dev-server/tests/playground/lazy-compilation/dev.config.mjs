import path from 'node:path';
import url from 'node:url';
import { defineDevConfig } from '@rolldown/test-dev-server';
import { viteAliasPlugin } from 'rolldown/experimental';

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));

// One shared config for every lazy-compilation scenario. Each scenario lives in
// its own sub-folder and is imported by `main.js`; because compilation is lazy,
// bundling them into one project never warms another scenario's chunks — a lazy
// chunk is compiled only when its own dynamic import fires. So a spec that clicks
// only its scenario's button gets a virgin first-fetch for that scenario, exactly
// as the four separate servers used to give.
//
// The config is the union of what each scenario needs:
// - `viteAliasPlugin` maps `@lazy` for the aliased-import scenario (inert for the
//   others, which never import `@lazy`) — regression for vitejs/vite#22454.
export default defineDevConfig({
  platform: 'browser',
  build: {
    input: { main: 'main.js' },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: { lazy: true },
    },
    plugins: [
      viteAliasPlugin({
        entries: [
          { find: '@lazy', replacement: path.resolve(__dirname, 'aliased-import/lazy.js') },
        ],
      }),
    ],
  },
});
