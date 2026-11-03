import path from 'node:path';
import { defineDevConfig } from '@rolldown/test-dev-server';

// Exercises the `hotUpdate` chain across TWO plugins: each plugin receives the
// set as edited by the plugins before it, not the engine default.
//
// - `readd.txt`: the first plugin returns `[]` (suppress). The second plugin
//   must see that empty set and can put `dep.js` back — an earlier empty
//   return is not final.
// - `keep.txt`: the first plugin replaces the set with `dep.js`. The second
//   plugin must see that replacement (not the default `main.js`) and declines,
//   which keeps the first plugin's choice.
//
// Both control files map to `main.js` by default (registered as transform
// dependencies below). Contract violations throw, which fails the round and
// times out the spec's accept-count poll.
const firstPlugin = {
  name: 'test-hot-update-chain-first',
  applyToEnvironment() {
    return {
      name: 'test-hot-update-chain-first:rolldown',
      transform: {
        filter: { id: /main\.js$/ },
        handler(_code, id) {
          this.addWatchFile(path.join(path.dirname(id), 'readd.txt'));
          this.addWatchFile(path.join(path.dirname(id), 'keep.txt'));
          return null;
        },
      },
      hotUpdate(ctx) {
        const expectDefault = () => {
          if (
            !(ctx.modules.length === 1 && ctx.modules[0].endsWith('main.js'))
          ) {
            throw new Error(
              `first plugin expected default modules [main.js], got ${JSON.stringify(ctx.modules)}`,
            );
          }
        };
        if (ctx.file.endsWith('readd.txt')) {
          expectDefault();
          return [];
        }
        if (ctx.file.endsWith('keep.txt')) {
          expectDefault();
          return [path.join(path.dirname(ctx.file), 'dep.js')];
        }
      },
    };
  },
};

const secondPlugin = {
  name: 'test-hot-update-chain-second',
  applyToEnvironment() {
    return {
      name: 'test-hot-update-chain-second:rolldown',
      hotUpdate(ctx) {
        if (ctx.file.endsWith('readd.txt')) {
          if (ctx.modules.length !== 0) {
            throw new Error(
              `second plugin expected the first plugin's empty set, got ${JSON.stringify(ctx.modules)}`,
            );
          }
          return [path.join(path.dirname(ctx.file), 'dep.js')];
        }
        if (ctx.file.endsWith('keep.txt')) {
          if (
            !(ctx.modules.length === 1 && ctx.modules[0].endsWith('dep.js'))
          ) {
            throw new Error(
              `second plugin expected the first plugin's replacement [dep.js], got ${JSON.stringify(ctx.modules)}`,
            );
          }
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
    plugins: [firstPlugin, secondPlugin],
  },
});
