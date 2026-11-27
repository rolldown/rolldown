import assert from 'node:assert/strict';
import { Capture as readOutside } from './dep.js';

function simple(Capture = () => {}) {
  return [readOutside(), Capture.name];
}

function destructured({ Capture = () => {} } = {}) {
  return [readOutside(), Capture.name];
}

assert.deepEqual(
  [simple(), destructured()],
  [
    ['outside', 'Capture'],
    ['outside', 'Capture'],
  ],
);
