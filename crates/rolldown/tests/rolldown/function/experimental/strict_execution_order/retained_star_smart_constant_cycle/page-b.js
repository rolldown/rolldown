import { x, y } from './bridge.js';

x();
globalThis.seenY = y;
if (globalThis.value !== undefined) {
  throw new Error(`page-b observed ${globalThis.value}`);
}

export function render() {}
