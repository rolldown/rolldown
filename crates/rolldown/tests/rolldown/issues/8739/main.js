import * as dep from './dep.js';
import assert from 'node:assert';

function test(head, tail) {
  let clashingIdentifier = head;
  let middle = dep.clashingIdentifier();
  let clashingIdentifier$1 = tail;
  return clashingIdentifier + middle + clashingIdentifier$1;
}

assert.strictEqual(test('head', 'tail'), 'headmiddletail');
