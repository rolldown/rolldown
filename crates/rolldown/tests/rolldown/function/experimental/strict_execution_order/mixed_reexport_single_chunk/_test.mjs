import assert from 'node:assert/strict';
import * as entry from './dist/main.js';

assert.deepEqual({ ...entry }, { LOCAL: 'local', VALUE: 'value' });
