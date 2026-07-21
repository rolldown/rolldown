export * from './bridge.js';

if (globalThis.value !== 0) {
  throw new Error(`page-a observed ${globalThis.value}`);
}
