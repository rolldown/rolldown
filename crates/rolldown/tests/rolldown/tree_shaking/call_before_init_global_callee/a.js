import { f } from './b.js';

class Skip {}

// In Node, ready.js runs first, so the getter of `gate` throws before the argument runs. In the
// output, this module runs first, so the argument runs there, and it must not throw.
Object.defineProperty(globalThis, 'gate', {
  configurable: true,
  get() {
    if (typeof ready !== 'undefined') throw new Skip();
    return () => {};
  },
});
try {
  gate((f(), 0));
} catch (error) {
  if (!(error instanceof Skip)) throw error;
}
try {
  gate((later(), 0));
} catch (error) {
  if (!(error instanceof Skip)) throw error;
}

const later = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('later');
globalThis.calls.push('a done');
