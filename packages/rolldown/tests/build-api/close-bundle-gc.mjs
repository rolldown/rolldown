// Each case builds with a plugin whose `closeBundle` captures a marker, closes the build, and
// drops every reference to it. After GC, no marker may stay alive. Each case except the control
// makes JS hold a native object that keeps the options alive (cached or kept options, a kept
// plugin context). Before the fix, the JS functions of the options were GC roots, so that native
// object made a cycle that the GC could not free (https://github.com/rolldown/rolldown/issues/11444).
// See internal-docs/bundler-data-lifecycle/implementation.md ("JS callback lifetime").
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { setTimeout } from 'node:timers/promises';
import { rolldown, watch } from 'rolldown';
import { dev } from 'rolldown/experimental';

assert(globalThis.gc, 'Run with --expose-gc');

const input = path.join(import.meta.dirname, 'main.js');
// Under the wasm binding, the dev engine close tears down the shared async runtime.
// See https://github.com/rolldown/rolldown/issues/10639.
const isWasi = process.env.ROLLDOWN_WASI_TEST === '1';

async function collect() {
  for (let i = 0; i < 12; i++) {
    // WeakRef.deref() keeps its target alive until the current job ends.
    await setTimeout(10);
    globalThis.gc();
  }
}

function markerPlugin(marker, hooks = {}) {
  const plugin = {
    name: 'marker',
    closeBundle() {
      marker.closed = true;
    },
    ...hooks,
  };
  return plugin;
}

// The plugin keeps the options that a hook gets. Then JS holds a `BindingNormalizedOptions`.
function keepOptions(hookName, marker, hooks = {}) {
  const plugin = markerPlugin(marker, hooks);
  plugin[hookName] = function (options) {
    plugin.options = options;
  };
  return plugin;
}

async function classic(inputOptions, outputOptions = {}, { generateCount = 1 } = {}) {
  const bundle = await rolldown({ input, logLevel: 'silent', ...inputOptions });
  try {
    for (let i = 0; i < generateCount; i++) {
      await bundle.generate({ format: 'es', ...outputOptions });
    }
  } finally {
    await bundle.close();
  }
}

const cases = {
  async control(marker) {
    await classic({ plugins: [markerPlugin(marker)] });
  },
  async 'empty buildStart hook'(marker) {
    await classic({ plugins: [markerPlugin(marker, { buildStart() {} })] });
  },
  async 'empty renderStart hook'(marker) {
    await classic({ plugins: [markerPlugin(marker, { renderStart() {} })] });
  },
  async 'plugin keeps the input options'(marker) {
    await classic({ plugins: [keepOptions('buildStart', marker)] });
  },
  async 'plugin keeps its plugin context'(marker) {
    const plugin = markerPlugin(marker);
    plugin.buildStart = function () {
      plugin.context = this;
    };
    await classic({ plugins: [plugin] });
  },
  async 'external function captures the plugin'(marker) {
    const plugin = keepOptions('buildStart', marker);
    await classic({ plugins: [plugin], external: (id) => Boolean(plugin) && id === 'never' });
  },
  async 'onLog function captures the plugin'(marker) {
    const plugin = keepOptions('buildStart', marker);
    await classic({ plugins: [plugin], onLog: () => void plugin });
  },
  async 'banner function captures the plugin'(marker) {
    const plugin = keepOptions('renderStart', marker);
    await classic({ plugins: [plugin] }, { banner: () => (plugin ? '' : '') });
  },
  async 'entryFileNames function captures the plugin'(marker) {
    const plugin = keepOptions('renderStart', marker);
    await classic({ plugins: [plugin] }, { entryFileNames: () => (plugin ? '[name].js' : '') });
  },
  async 'code splitting group name function captures the plugin'(marker) {
    const plugin = keepOptions('renderStart', marker);
    await classic(
      { plugins: [plugin] },
      { codeSplitting: { groups: [{ name: () => (plugin ? 'group' : null) }] } },
    );
  },
  async 'code splitting group test function captures the plugin'(marker) {
    const plugin = keepOptions('renderStart', marker);
    await classic(
      { plugins: [plugin] },
      { codeSplitting: { groups: [{ name: 'group', test: () => Boolean(plugin) && false }] } },
    );
  },
  async 'two generate calls, one close'(marker) {
    await classic({ plugins: [keepOptions('buildStart', marker)] }, {}, { generateCount: 2 });
  },
  async 'failed build, then close'(marker) {
    await assert.rejects(
      classic({
        input: path.join(import.meta.dirname, 'missing.js'),
        plugins: [keepOptions('buildStart', marker)],
      }),
    );
  },
  async 'throwing closeBundle hook'(marker) {
    const plugin = keepOptions('buildStart', marker);
    plugin.closeBundle = () => {
      marker.closed = true;
      throw new Error('closeBundle failed');
    };
    await assert.rejects(classic({ plugins: [plugin] }), /closeBundle failed/);
  },
  async 'watch mode'(marker) {
    const watcher = watch({
      input,
      logLevel: 'silent',
      plugins: [keepOptions('buildStart', marker)],
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
  async 'watch mode, watcher closes while an event result close runs'(marker) {
    let hookStarted;
    const started = new Promise((resolve) => (hookStarted = resolve));
    let releaseHook;
    const paused = new Promise((resolve) => (releaseHook = resolve));
    const plugin = keepOptions('buildStart', marker);
    plugin.closeBundle = async () => {
      hookStarted();
      await paused;
      marker.closed = true;
    };
    const watcher = watch({
      input,
      logLevel: 'silent',
      external: (id) => Boolean(plugin) && id === 'never',
      plugins: [plugin],
      watch: { skipWrite: true },
    });
    let onResultClosed;
    const resultClosed = new Promise((resolve) => (onResultClosed = resolve));
    watcher.on('event', (event) => {
      if (event.code === 'BUNDLE_END' || event.code === 'ERROR') {
        event.result.close().then(onResultClosed);
      }
    });
    await started;
    await watcher.close();
    releaseHook();
    await resultClosed;
  },
  async 'dev engine'(marker) {
    if (isWasi) return;
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-close-gc-'));
    try {
      const engine = await dev(
        {
          input,
          logLevel: 'silent',
          experimental: { devMode: true },
          plugins: [keepOptions('buildStart', marker)],
        },
        { dir },
        { watch: { enabled: false } },
      );
      engine.run().catch(() => {});
      await engine.ensureCurrentBuildFinish();
      await engine.close();
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  },
  async 'dev engine, closeBundle awaits engine.close()'(marker) {
    if (isWasi) return;
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-close-gc-'));
    try {
      const plugin = keepOptions('buildStart', marker);
      // A plugin that holds the engine itself keeps the engine and its plugin hooks alive, and the
      // release does not cover plugin hooks. So the hook reaches the engine only through a WeakRef.
      let engineRef;
      plugin.closeBundle = async () => {
        const running = engineRef.deref();
        assert(running, 'The engine is alive during its close');
        await running.close();
        marker.closed = true;
      };
      const engine = await dev(
        {
          input,
          logLevel: 'silent',
          external: (id) => Boolean(plugin) && id === 'never',
          experimental: { devMode: true },
          plugins: [plugin],
        },
        { dir },
        { watch: { enabled: false } },
      );
      engineRef = new WeakRef(engine);
      engine.run().catch(() => {});
      await engine.ensureCurrentBuildFinish();
      await engine.close();
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  },
};

async function runCase(run) {
  const marker = {};
  await run(marker);
  return new WeakRef(marker);
}

const references = [];
for (const [name, run] of Object.entries(cases)) {
  references.push({ name, reference: await runCase(run) });
}
await collect();
const retained = references.filter(({ reference }) => reference.deref() !== undefined);
assert.deepEqual(
  retained.map(({ name }) => name),
  [],
  'The markers of these closed builds stay alive',
);
