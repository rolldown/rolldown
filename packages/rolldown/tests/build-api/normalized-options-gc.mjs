// Each case builds with a plugin that captures a marker, then returns. After that, the script keeps
// only a `WeakRef` to the marker and checks that the GC frees it.
//
// In every case except the control, a hook gets the normalized options. Those objects must not keep
// the native options alive: the native options own the JS option functions, which are GC roots
// that reach the options objects again (https://github.com/rolldown/rolldown/issues/11444).
// See internal-docs/bundler-data-lifecycle/implementation.md ("Normalized options objects").
import assert from 'node:assert/strict';
import path from 'node:path';
import { setTimeout } from 'node:timers/promises';
import { rolldown, watch } from 'rolldown';

assert(globalThis.gc, 'Run with --expose-gc');

const input = path.join(import.meta.dirname, 'main.js');

function markerPlugin(marker, hooks = {}) {
  return {
    name: 'marker',
    closeBundle() {
      marker.closed = true;
    },
    ...hooks,
  };
}

async function closedBuild(plugin) {
  const bundle = await rolldown({ input, logLevel: 'silent', plugins: [plugin] });
  try {
    await bundle.generate({ format: 'es' });
  } finally {
    await bundle.close();
  }
}

const cases = {
  async 'control: no hook gets the options'(marker) {
    await closedBuild(markerPlugin(marker));
  },
  async 'buildStart gets the input options'(marker) {
    await closedBuild(markerPlugin(marker, { buildStart(_options) {} }));
  },
  async 'renderStart and generateBundle get the output options'(marker) {
    await closedBuild(
      markerPlugin(marker, { renderStart(_outputOptions) {}, generateBundle(_outputOptions) {} }),
    );
  },
  async 'buildStart gets the input options, build never closed'(marker) {
    const bundle = await rolldown({
      input,
      logLevel: 'silent',
      plugins: [
        {
          name: 'marker',
          buildStart() {
            marker.started = true;
          },
        },
      ],
    });
    await bundle.generate({ format: 'es' });
  },
  async 'watch mode, event result and watcher closed'(marker) {
    const watcher = watch({
      input,
      logLevel: 'silent',
      plugins: [markerPlugin(marker, { buildStart(_options) {} })],
      watch: { skipWrite: true },
    });
    await new Promise((resolve, reject) => {
      watcher.on('event', async (event) => {
        if (event.code === 'BUNDLE_END') {
          await event.result.close();
          resolve();
        } else if (event.code === 'ERROR') {
          await event.result.close();
          reject(event.error);
        }
      });
    });
    await watcher.close();
  },
};

// The marker and every other local of a case live only in this function scope.
async function runCase(run) {
  const marker = {};
  await run(marker);
  return new WeakRef(marker);
}

const references = [];
for (const [name, run] of Object.entries(cases)) {
  references.push({ name, reference: await runCase(run) });
}

// A freed native object releases the option functions in a later event loop turn, so one `gc()`
// is not enough. Loop until every marker is freed or the round budget is used.
let retained = references;
for (let round = 1; round <= 30 && retained.length > 0; round++) {
  // WeakRef.deref() keeps its target alive until the current job ends.
  await setTimeout(10);
  globalThis.gc();
  retained = retained.filter(({ reference }) => reference.deref() !== undefined);
}
assert.deepEqual(
  retained.map(({ name }) => name),
  [],
  'The markers of these builds stay alive',
);
