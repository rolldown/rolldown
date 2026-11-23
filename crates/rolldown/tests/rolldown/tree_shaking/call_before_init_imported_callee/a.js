import { gate } from './gate.js';
import { f } from './b.js';

// In Node, gate.js runs after this module, so the read of `gate` throws before the argument runs.
// In the output, gate.js runs first, so the argument runs there, and it must not throw.
try {
  gate((f(), 0));
} catch (error) {
  if (gateIsReady()) throw error;
}
try {
  gate((later(), 0));
} catch (error) {
  if (gateIsReady()) throw error;
}

const later = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('later');
globalThis.calls.push('a done');

function gateIsReady() {
  try {
    return typeof gate === 'function';
  } catch {
    return false;
  }
}
