import assert from 'node:assert';
import nodeFs from 'node:fs';
import { value } from './hmr-boundary';

assert.equal(value, 1);

globalThis.hmrChange = async (exports) => {
  console.log('HMR change detected');
  if (exports.value === 2) {
    nodeFs.writeFileSync('./ok-0', '');
  }
  process.exit(0);
};
