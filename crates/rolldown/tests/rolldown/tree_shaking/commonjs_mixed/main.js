import { fromWebToken } from './lib';
import assert from 'node:assert'


(async () => {
  assert.strictEqual(
    await fromWebToken()(),
    1,
  );
})();



