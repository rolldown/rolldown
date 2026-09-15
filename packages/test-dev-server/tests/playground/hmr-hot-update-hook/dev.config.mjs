import path from 'node:path';
import { defineDevConfig } from '@rolldown/test-dev-server';

// Exercises the experimental `hotUpdate` plugin hook end-to-end through the browser:
// - `config.txt` REPLACES the affected set (main.js -> dep.js), so the client re-runs
//   dep.js and main's accept callback fires without a reload.
// - `suppress.txt` SUPPRESSES the update entirely (no client message at all).
// - `drop-some.txt` returns an unknown id next to dep.js: the unknown id is dropped
//   and dep.js still ships.
// - `drop-all.txt` returns only an unknown id: dropping it empties the set, which
//   ends the round as a noop, same as suppression.
// - `notes.txt` is watched from `buildStart` and belongs to no module: the chain
//   still runs with an EMPTY default set, and the hook can claim dep.js for it.
// - `dep.js` edits are DECLINED: the default flow must stay intact with a hook
//   registered, including the unchanged-output suppression for whitespace-only edits.
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
      buildStart() {
        // A plain watch registration (no transform dependency): notes.txt maps
        // to zero modules, so the hook must receive it with an empty set.
        this.addWatchFile(path.join(import.meta.dirname, 'notes.txt'));
      },
      transform: {
        filter: { id: /main\.js$/ },
        handler(_code, id) {
          // Watch the control files so the engine's default mapping points them at main.js.
          this.addWatchFile(path.join(path.dirname(id), 'config.txt'));
          this.addWatchFile(path.join(path.dirname(id), 'suppress.txt'));
          this.addWatchFile(path.join(path.dirname(id), 'drop-some.txt'));
          this.addWatchFile(path.join(path.dirname(id), 'drop-all.txt'));
          return null;
        },
      },
      hotUpdate(ctx) {
        const expectModules = (...names) => {
          if (
            !(ctx.modules.length === names.length &&
              ctx.modules.every((id, i) => id.endsWith(names[i])))
          ) {
            throw new Error(
              `expected modules [${names}], got ${JSON.stringify(ctx.modules)}`,
            );
          }
        };
        if (ctx.file.endsWith('config.txt')) {
          expectModules('main.js');
          return [path.join(path.dirname(ctx.file), 'dep.js')];
        }
        if (ctx.file.endsWith('suppress.txt')) {
          return [];
        }
        if (ctx.file.endsWith('drop-some.txt')) {
          expectModules('main.js');
          return [
            path.join(path.dirname(ctx.file), 'missing.js'),
            path.join(path.dirname(ctx.file), 'dep.js'),
          ];
        }
        if (ctx.file.endsWith('drop-all.txt')) {
          expectModules('main.js');
          return [path.join(path.dirname(ctx.file), 'missing.js')];
        }
        if (ctx.file.endsWith('notes.txt')) {
          expectModules(); // unmapped file: empty default set
          return [path.join(path.dirname(ctx.file), 'dep.js')];
        }
        if (ctx.file.endsWith('dep.js')) {
          expectModules('dep.js');
          // Decline: dep's own edits go through the default flow.
          return undefined;
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
