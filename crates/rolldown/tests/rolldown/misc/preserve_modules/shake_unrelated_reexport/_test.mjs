import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const wrapper = fs.readFileSync(path.join(import.meta.dirname, 'dist/wrapper.js'), 'utf8');

assert(
  !/\bimport\s*\{[^}]*\bfoo\b[^}]*\}\s*from\s*["']\.\/a\.js["']/.test(wrapper),
  'wrapper.js should not import foo from a.js for an unused re-export',
);

assert(
  !/\bexport\s*\{\s*foo\s*\}/.test(wrapper),
  'wrapper.js should not export unused re-export facade foo',
);
