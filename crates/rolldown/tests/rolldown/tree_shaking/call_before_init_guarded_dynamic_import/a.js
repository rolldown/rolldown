import { f } from './b.js';

class Skip {}

// Node runs ready.js before this module, so the calls of `f` and the last call of `later` do not
// run there. The output runs this module first, so they run there, and they must not throw.
if (typeof ready === 'undefined') f();
try {
  if (typeof ready !== 'undefined') throw new Skip();
  f();
} catch (error) {
  if (!(error instanceof Skip)) throw error;
}
// In Node, the getter of `gate` throws before the argument runs. In the output, the read succeeds,
// so the argument runs there, and it must not throw.
Object.defineProperty(globalThis, 'gate', {
  configurable: true,
  get() {
    if (typeof ready !== 'undefined') throw new Skip();
    return () => {};
  },
});
try {
  gate((later(), 0));
} catch (error) {
  if (!(error instanceof Skip)) throw error;
}
// This call throws in Node too, but the guarded call below must still not throw.
try {
  later();
} catch (error) {
  globalThis.calls.push(`same module: ${error.name}`);
}
if (typeof ready === 'undefined') later();

const later = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('later');
globalThis.calls.push('a done');
