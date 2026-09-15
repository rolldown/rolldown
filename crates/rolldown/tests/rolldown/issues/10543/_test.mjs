import assert from 'node:assert';

// #10543: `styles` re-exports `V`/`P`/`util` from a subtree that is ESM-wrapped
// (`lab.cjs` requires `top.js`; wrapping propagates to `system.js`, `mid.js`, `w.js`),
// `icon.js` pulls `w.js` into a wider shared chunk than the barrels', and every
// forwarding statement between the entry and `w.js` is tree-shaken (declared
// side-effect-free). The entry chunk must call the cross-chunk `init_w` itself;
// previously it was defined and exported but called nowhere and `V` stayed
// uninitialized.
const { P, V, util } = await import('./dist/styles.js');
assert.equal(V, 'v18');
assert.equal(P(), 'v18');
assert.equal(util, 1);
// ESM evaluates dependencies before the importer's body, so the entry body must
// already observe `w.js`'s side effect — the init calls have to precede the entry
// module's own statements, not sit at the chunk tail.
assert.equal(globalThis.__entry_body_saw_w, 'ran');
