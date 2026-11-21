import * as ns from './b.js';
import hoistedDefault, { emptyArrow, hoisted, letArrow, nse } from './b.js';
import { outside } from './c.js';

try {
  nse();
} catch (error) {
  globalThis.calls.push(`no side effects: ${error.name}`);
}
try {
  letArrow();
} catch (error) {
  globalThis.calls.push(`let: ${error.name}`);
}
try {
  emptyArrow();
} catch (error) {
  globalThis.calls.push(`empty arrow: ${error.name}`);
}
try {
  ns.nse();
} catch (error) {
  globalThis.calls.push(`namespace: ${error.name}`);
}
hoisted();
hoistedDefault();
outside();
