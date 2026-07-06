import assert from 'node:assert';

// Regression test for #9993 (a regression of #9224): the runtime module must
// not be merged into a chunk that is part of a static import cycle with a
// chunk calling runtime helpers (e.g. `__commonJSMin`) at top level. Before
// the fix, the entry chunk of `entry-2` hosted the runtime while the group
// chunk `v.js` both consumed `__commonJSMin` at top level and was statically
// imported back by `entry-2.js` (entry-2's entry module was captured into the
// group chunk), so evaluating `entry-2.js` threw
// `TypeError: __commonJSMin is not a function`.
//
// Note: entry-2 must be imported first here. Loading `entry-1.js` as the
// evaluation root still throws `require_node3 is not a function` because
// entry-2's facade calls the group chunk's top-level `require_node3` binding
// before that chunk's body runs. That cross-chunk evaluation-order defect is
// NOT part of the #9993 regression window — rolldown 1.0.3 emits the same
// entry-1-first crash for this input.
const entry2 = await import('./dist/entry-2.js');
assert.strictEqual(entry2.default.node_3, 3);
const entry1 = await import('./dist/entry-1.js');
assert.strictEqual(entry1.default.node_4, 4);
assert.strictEqual(globalThis.__issue_9993_3, 1);
assert.strictEqual(globalThis.__issue_9993_4, 1);
assert.strictEqual(globalThis.__issue_9993_5, 1);
