import { strict as assert } from 'node:assert';
import fs from 'node:fs';
const code = fs.readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');
const lib =
  globalThis.__configName === 'iife'
    ? new Function(code + '; return lib;')()
    : (() => {
        const module = { exports: {} };
        new Function('module', 'exports', 'require', code)(module, module.exports, () => {});
        return module.exports;
      })();
assert.equal(lib.s, 'local');
assert.equal(Object.prototype.toString.call(lib), '[object Module]');
