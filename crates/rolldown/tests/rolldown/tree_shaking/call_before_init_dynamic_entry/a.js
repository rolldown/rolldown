import { clause, declared } from './b.js';
import { noop } from './c.js';

try {
  declared();
} catch (error) {
  globalThis.calls.push(`declared: ${error.name}`);
}
try {
  noop(declared());
} catch (error) {
  globalThis.calls.push(`declared argument: ${error.name}`);
}
try {
  clause();
} catch (error) {
  globalThis.calls.push(`clause: ${error.name}`);
}
try {
  noop(clause());
} catch (error) {
  globalThis.calls.push(`clause argument: ${error.name}`);
}
