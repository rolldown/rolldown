import nodeFs from 'node:fs';
import { defineDevConfig } from '@rolldown/test-dev-server';

// Exercises a THROWING `hotUpdate` hook end-to-end. The hook fails whenever
// dep.js's on-disk content says 'dep-v2', so:
//
// - the v1 -> v2 edit errors the whole round (overlay in the browser, dep
//   still renders v1);
// - a later edit to other.js ships alone — the failed round's dep edit is
//   NOT queued for retry (unlike a failed scan, a hook error adds nothing to
//   `pending_rescans`);
// - dep's content only reaches the browser when dep.js itself changes again
//   (v3), which also clears the error.
const errorHookPlugin = {
  name: 'test-hot-update-error',
  applyToEnvironment() {
    return {
      name: 'test-hot-update-error:rolldown',
      hotUpdate(ctx) {
        if (
          ctx.file.endsWith('dep.js') &&
          nodeFs.readFileSync(ctx.file, 'utf-8').includes('dep-v2')
        ) {
          throw new Error('hotUpdate hook failed on purpose');
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
    plugins: [errorHookPlugin],
  },
});
