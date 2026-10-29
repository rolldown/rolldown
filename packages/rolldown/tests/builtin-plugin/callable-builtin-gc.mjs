import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';
const { viteResolvePlugin } = await import(process.argv[2] ?? 'rolldown/experimental');

assert(global.gc, 'Run with --expose-gc');

function createPlugin(callbacks) {
  return viteResolvePlugin({
    resolveOptions: {
      isBuild: false,
      isProduction: true,
      asSrc: true,
      preferRelative: false,
      root: import.meta.dirname,
      scan: false,
      mainFields: ['module', 'main'],
      conditions: [],
      externalConditions: [],
      extensions: ['.js', '.json'],
      tryIndex: true,
      preserveSymlinks: false,
      tsconfigPaths: false,
    },
    environmentConsumer: 'client',
    environmentName: 'client',
    builtins: [],
    external: [],
    noExternal: [],
    dedupe: [],
    resolveSubpathImports: () => undefined,
    ...callbacks,
  });
}

async function collect() {
  for (let i = 0; i < 12; i++) {
    // WeakRef.deref() keeps its target alive until the current job ends.
    await setTimeout(10);
    global.gc();
  }
}

function releasedOwner(key, captureOwner) {
  const owner = {};
  owner.plugin = createPlugin({
    [key]: captureOwner
      ? () => {
          owner.called = true;
        }
      : console.warn,
  });
  return new WeakRef(owner);
}

// Each option creates a separate native callback root, even without a hook call.
const keys = [
  'onWarn',
  'onDebug',
  'resolveSubpathImports',
  'finalizeBareSpecifier',
  'finalizeOtherSpecifiers',
];
const released = keys.map((key) => ({
  key,
  control: releasedOwner(key, false),
  captured: releasedOwner(key, true),
}));
await collect();
for (const { key, control, captured } of released) {
  assert.equal(control.deref(), undefined, `${key}: control owner retained`);
  assert.equal(captured.deref(), undefined, `${key}: callback owner retained`);
}

function detachedHook() {
  const owner = { warnings: 0, subpaths: 0 };
  owner.plugin = createPlugin({
    onWarn() {
      owner.warnings++;
    },
    resolveSubpathImports() {
      owner.subpaths++;
      return 'node:fs';
    },
  });
  return { hook: owner.plugin.resolveId, reference: new WeakRef(owner) };
}

const detached = detachedHook();
await detached.hook('#check', import.meta.filename);
await collect();
await detached.hook('#check', import.meta.filename);
assert.equal(detached.reference.deref().warnings, 2);
assert.equal(detached.reference.deref().subpaths, 2);
detached.hook = undefined;
await collect();
assert.equal(detached.reference.deref(), undefined, 'Detached hook owner retained');

function changedOptions(mode) {
  const owner = { warnings: 0 };
  owner.plugin = createPlugin({
    onWarn() {
      owner.warnings++;
    },
  });
  if (mode === 'replace') owner.plugin._options = {};
  else if (mode === 'delete') delete owner.plugin._options;
  else
    owner.plugin._options.onWarn = () => {
      throw new Error('replacement callback called');
    };
  return { hook: owner.plugin.resolveId, reference: new WeakRef(owner) };
}
const changed = ['replace', 'delete', 'mutate'].map(changedOptions);
await collect();
for (const entry of changed) {
  await entry.hook('node:fs', import.meta.filename);
  assert.equal(entry.reference.deref().warnings, 1);
  entry.hook = undefined;
}
await collect();
for (const entry of changed) {
  assert.equal(entry.reference.deref(), undefined, 'Owner retained after _options mutation');
}

for (const kind of ['proxy', 'null-prototype']) {
  let calls = 0;
  const callback = function (message) {
    assert.equal(this, undefined);
    assert.equal(typeof message, 'string');
    calls++;
  };
  const onWarn =
    kind === 'proxy'
      ? new Proxy(callback, {
          get() {
            throw new Error('callback property read');
          },
        })
      : Object.setPrototypeOf(callback, null);
  const plugin = createPlugin({ onWarn });
  await plugin.resolveId('node:fs', import.meta.filename);
  assert.equal(calls, 1);
}

const throwing = createPlugin({
  resolveSubpathImports() {
    throw new Error('subpath callback failed');
  },
  async onWarn() {
    throw new Error('warning callback failed');
  },
});
await assert.rejects(throwing.resolveId('#error', import.meta.filename), /subpath callback failed/);
await assert.rejects(
  throwing.resolveId('node:fs', import.meta.filename),
  /warning callback failed/,
);
