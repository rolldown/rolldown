import assert from 'node:assert';
import { cssFns } from './css.js';

assert.strictEqual(globalThis.colorLoaded, 1);
export const theme = cssFns.setOpacity('red', 0.5);
