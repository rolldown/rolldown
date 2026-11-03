import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

if (globalThis.__configName === 'cjs') {
  const name = '\n"\\\'';
  const external = { [name]: 42 };
  const exports = {};
  const code = readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');

  runInNewContext(code, {
    exports,
    require(id) {
      assert.strictEqual(id, 'external');
      return external;
    },
  });

  assert.strictEqual(exports[name], 42);
  external[name] = 43;
  assert.strictEqual(exports[name], 43);
}
