import assert from 'assert';
import json from './dist/main';

assert.deepEqual(
  json.foo,
  '__EXP__',
  'JSON import should match expected value',
);
