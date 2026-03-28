// Reproduces a bug where inline const incorrectly inlines `void 0` for a CJS
// named export when the CJS entry conditionally re-exports from two files,
// and one of them (production) sets the export to `void 0`.
// This mimics the react/jsx-dev-runtime pattern.
import assert from 'node:assert';
import { jsxDEV } from './jsx-dev-runtime.js';

const show = true;
const result = show && jsxDEV('span', { children: 'hello' });
assert.deepStrictEqual(result, { type: 'span', props: { children: 'hello' } });

import.meta.hot.accept();
