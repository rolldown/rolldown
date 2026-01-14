import assert from 'node:assert';
import config from './dist/config.js';

// The config should be { name: 'example' }, not undefined
assert.deepStrictEqual(config, { name: 'example' });
