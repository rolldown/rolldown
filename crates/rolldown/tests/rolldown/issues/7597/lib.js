import assert from 'node:assert';
import CloseButton from './b.js';
import Transition from './a.js';

assert.strictEqual(CloseButton, 'b', 'CloseButton should be "b"');
assert.strictEqual(Transition, 'a', 'Transition should be "a"');

let Modal = {};

export { Modal };
