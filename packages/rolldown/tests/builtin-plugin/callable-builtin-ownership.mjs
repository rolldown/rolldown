import assert from 'node:assert/strict';
import { setTimeout } from 'node:timers/promises';

const { BuiltinPlugin, makeBuiltinPluginCallable, nativeRoots, setPending } = await import(
  process.argv[2]
);
assert(global.gc, 'Run with --expose-gc');

async function collect() {
  for (let i = 0; i < 12; i++) {
    await setTimeout(10);
    global.gc();
  }
}

function createOwner() {
  const owner = { calls: 0 };
  owner.plugin = makeBuiltinPluginCallable(
    new BuiltinPlugin(
      'builtin:vite-resolve',
      Object.freeze({
        onWarn() {
          owner.calls++;
        },
      }),
    ),
  );
  return owner;
}

function releasedOwner() {
  return new WeakRef(createOwner());
}
const released = releasedOwner();
await collect();
assert.equal(released.deref(), undefined, 'Released callback owner retained');
assert.throws(nativeRoots.at(-1), /The callback for builtin:vite-resolve.onWarn was released/);

function createGate() {
  let resume;
  const promise = new Promise((resolve) => {
    resume = resolve;
  });
  return { promise, resume };
}
function pendingOwner() {
  const owner = createOwner();
  const result = owner.plugin.resolveId('test');
  delete owner.plugin.resolveId;
  delete owner.plugin._options;
  return { result, reference: new WeakRef(owner) };
}
const gate = createGate();
setPending(gate.promise);
const pending = pendingOwner();
await collect();
const retained = pending.reference.deref() !== undefined;
gate.resume();
const result = await pending.result.then(
  () => undefined,
  (error) => error,
);
assert(retained, 'Pending hook lost its callback owner');
assert.equal(result, undefined);
assert.equal(pending.reference.deref().calls, 1);
await collect();
assert.equal(pending.reference.deref(), undefined, 'Completed hook owner retained');
