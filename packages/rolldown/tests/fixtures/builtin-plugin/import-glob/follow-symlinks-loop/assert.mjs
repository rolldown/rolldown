// @ts-nocheck
import assert from 'node:assert';
import { modules } from './dist/main';

assert.deepStrictEqual(Object.keys(modules).sort(), ['./dir/a.js', './dir/sub/b.js']);
