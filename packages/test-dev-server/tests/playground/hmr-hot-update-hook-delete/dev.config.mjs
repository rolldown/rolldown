import nodeFs from 'node:fs';
import path from 'node:path';
import { defineDevConfig } from '@rolldown/test-dev-server';

// Exercises `hotUpdate` for DELETE events: the deleted module itself must
// arrive in the hook's default set, a decline must fall back to the default
// delete flow, and a replacement returned for a delete must ship as-is.
//
// The spec first edits main.js to drop both child imports, so the deletes hit
// orphaned modules (deleting a still-imported file is a resolve error — that
// scenario is `hmr-delete-self-watched`'s job). `buildStart` watches both
// children so their delete events still reach the engine after they leave the
// graph.
//
// A noop is also what a lost event looks like, so the hook appends every
// delete it sees to `hook-log.txt`; the spec reads the log to prove the hook
// really ran with the right arguments. The log file itself is never watched
// and maps to no module, so writing it does not disturb the rounds.
const logPath = path.join(import.meta.dirname, 'hook-log.txt');

const deleteHookPlugin = {
  name: 'test-hot-update-delete',
  applyToEnvironment() {
    return {
      name: 'test-hot-update-delete:rolldown',
      buildStart() {
        this.addWatchFile(path.join(import.meta.dirname, 'child-a.js'));
        this.addWatchFile(path.join(import.meta.dirname, 'child-b.js'));
      },
      hotUpdate(ctx) {
        const isChild = ctx.file.endsWith('child-a.js') || ctx.file.endsWith('child-b.js');
        if (!isChild) {
          return;
        }
        const bases = ctx.modules.map((id) => path.basename(id));
        nodeFs.appendFileSync(
          logPath,
          `${ctx.type} ${path.basename(ctx.file)} ${JSON.stringify(bases)}\n`,
        );
        if (ctx.type === 'delete' && ctx.file.endsWith('child-b.js')) {
          // Replace-on-delete: ship main.js directly instead of the default
          // delete flow. Main's own code is unchanged — it must ship anyway
          // (hook-returned modules skip the unchanged-output suppression).
          return [path.join(path.dirname(ctx.file), 'main.js')];
        }
        // child-a.js: decline — the default delete flow applies.
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
    plugins: [deleteHookPlugin],
  },
});
