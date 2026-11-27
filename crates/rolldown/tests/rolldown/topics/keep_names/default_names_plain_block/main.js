import assert from 'node:assert/strict';
import { Capture as outside } from './dep.js';

{
  const Capture = () => {};
  assert.deepEqual([outside, Capture.name], ['outside', 'Capture']);
}
