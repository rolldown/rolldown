import assert from 'node:assert/strict';
import { loadA, loadB } from './dist/main.js';

assert.equal((await loadA()).A(), 'a');
assert.equal((await loadB()).B(), 'bi');
