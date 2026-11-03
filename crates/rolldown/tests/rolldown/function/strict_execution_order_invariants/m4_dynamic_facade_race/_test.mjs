import assert from 'node:assert';

const { targetPromise } = await import('./dist/a.js');
await targetPromise;
// The trigger rides the `import()` rewrite rather than a facade file, so it runs in the rewrite's
// `.then` — one microtask after the host chunk settles. A microtask already queued when the chunk
// settles therefore observes the target uninitialized. What the importer sees is unaffected: by the
// time its own `await` resumes, the target has run.
assert.deepStrictEqual(
  globalThis.events,
  ['checkpoint:false', 'target'],
  'the checkpoint runs before the call-site trigger; the importer still never sees an uninitialized target',
);
await import('./dist/b.js');

assert.deepStrictEqual(
  globalThis.events,
  ['checkpoint:false', 'target', 'observer:true'],
  'entry b must still initialize the target before the observer',
);
