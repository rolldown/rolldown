import assert from 'node:assert/strict';
import { url } from './dist/main.js';

assert.equal(url, new URL('./dist/m.wasm', import.meta.url).href);
