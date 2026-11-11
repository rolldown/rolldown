import assert from 'node:assert';
import paths from './paths.cjs';

assert.deepEqual(paths, [__dirname, __filename]);
