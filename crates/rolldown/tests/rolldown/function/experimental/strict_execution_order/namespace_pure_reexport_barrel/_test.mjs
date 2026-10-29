import assert from 'node:assert';

globalThis.__events = [];

const page = await import('./dist/main.js');
const loaded = await page.loadPage();

// The reader captured the barrel namespace at init but reads a member only now, through this
// deferred call (like recharts' render-time `scales[realScaleType]()`). The factory reads the
// definer's module-level `unit`, which is only assigned when the definer's `init_*()` runs. Under
// strict execution order the barrel's own `init_*` must forward to the side-effect-free definer's
// `init_*` at init time. The regression left the barrel init empty, so `unit` stayed `undefined`
// and the scale reads back `NO_UNIT`.
const result = loaded.getScale('scaleLinear');
assert.strictEqual(
  result,
  7,
  `barrel must forward init to the pure definer so the scale reads its unit; got ${JSON.stringify(result)} events=${JSON.stringify(globalThis.__events)}`,
);
