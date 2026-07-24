import path from 'node:path';
import { defineDevConfig } from '@rolldown/test-dev-server';

// Exercises the experimental `hotUpdate` plugin hook end-to-end through the browser:
// editing `config.txt` must REPLACE the affected set (main.js -> dep.js), so the client
// re-runs dep.js and main's accept callback fires without a reload; editing `suppress.txt`
// must SUPPRESS the update entirely (no client message at all).
//
// The plugin speaks ROLLDOWN's `hotUpdate` contract (plain module ids in and
// out), so it enters the bundled environment through `applyToEnvironment` —
// the structural marker vite's bundled-dev adapter uses to leave the hook
// unwrapped. A top-level `hotUpdate` would be wrapped with vite's
// `EnvironmentModuleNode` contract instead (covered by the
// `hmr-hot-update-hook-vite` playground).
const hotUpdateTestPlugin = {
  name: 'test-hot-update-hook',
  applyToEnvironment() {
    return {
      name: 'test-hot-update-hook:rolldown',
      transform: {
        filter: { id: /main\.js$/ },
        handler(_code, id) {
          // Watch both control files so the engine's default mapping points them at main.js.
          this.addWatchFile(path.join(path.dirname(id), 'config.txt'));
          this.addWatchFile(path.join(path.dirname(id), 'suppress.txt'));
          return null;
        },
      },
      hotUpdate(ctx) {
        if (ctx.file.endsWith('config.txt')) {
          if (
            !(ctx.modules.length === 1 && ctx.modules[0].endsWith('main.js'))
          ) {
            throw new Error(
              `expected default modules [main.js], got ${JSON.stringify(ctx.modules)}`,
            );
          }
          return [path.join(path.dirname(ctx.file), 'dep.js')];
        }
        if (ctx.file.endsWith('suppress.txt')) {
          return [];
        }
      },
    };
  },
};

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: {
      main: 'main.js',
    },
    platform: 'browser',
    treeshake: false,
    experimental: {
      devMode: {},
    },
    plugins: [hotUpdateTestPlugin],
  },
});
