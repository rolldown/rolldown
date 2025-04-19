import assert from 'node:assert';
import { value } from './hmr-boundary';

assert(value, 1);

globalThis.hmrChange = (exports) => {
  assert(exports.value, 2);
  process.exit(0);
};
