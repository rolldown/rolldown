import assert from 'node:assert';
import { result, classLike } from './a.js';

export default function () {
  new classLike();
  assert.equal(result, 1);
}
