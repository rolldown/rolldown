import assert from 'node:assert';

// `main` uses top-level await. Rolldown used to flag the whole graph as TLA and
// disable `optimize_facade_entry_chunks`, keeping each dynamic-import target as
// its own re-export proxy chunk. Now TLA only blocks merges/reductions that would
// close an *awaited* dependency cycle, and these targets form none, so the proxies
// are eliminated: each `import('./x.js')` is rewritten to load the merged
// `shared-abc` chunk directly (see _config.json).
const [a, b, c] = await Promise.all([
  import('./a.js'),
  import('./b.js'),
  import('./c.js'),
]);

assert.strictEqual(a.A, 'a-payload');
assert.strictEqual(b.B, 'b-payload');
assert.strictEqual(c.C, 'c-payload');
