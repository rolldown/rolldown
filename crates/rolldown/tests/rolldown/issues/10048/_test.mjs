import assert from 'node:assert';
// #10048: importing the built entry must not throw. Before the fix the emitted
// chunk called `__commonJS(...)` / `__toESM(...)` without declaring or importing
// them (their defining module `helpers.js` was tree-shaken away), so this import
// threw `ReferenceError: __commonJS is not defined` at module-load time.
import { getProjectAnnotations } from './dist/entry.js';

assert.deepStrictEqual(getProjectAnnotations(), [
  {
    light: { base: 'light', appBg: '#ffffff' },
    dark: { base: 'dark', appBg: '#000000' },
  },
]);
