import { x } from './bridge.js';

x();
if (globalThis.value !== undefined) {
  throw new Error(`page-b observed ${globalThis.value}`);
}

export function render() {}
