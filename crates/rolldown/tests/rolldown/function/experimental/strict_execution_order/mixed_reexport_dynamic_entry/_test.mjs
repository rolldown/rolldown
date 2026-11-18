import assert from 'node:assert/strict';
import { read } from './dist/main.js';

assert.deepEqual(await read(), ['local', 'a', 'b']);
