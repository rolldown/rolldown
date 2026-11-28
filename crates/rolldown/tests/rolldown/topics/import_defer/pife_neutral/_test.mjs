import assert from 'node:assert';

globalThis.events = [];
const { run } = await import('./dist/main.js');
assert.deepStrictEqual(globalThis.events, []);
assert.deepStrictEqual(run(), ['lazy', 'required']);
assert.deepStrictEqual(globalThis.events, ['lazy', 'required']);
