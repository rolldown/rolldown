// MULTIPLE ENTRY MODULES
import { test as s, a as b } from './a.js';
import assert from 'assert'


s();
b();
const test = 10;
console.log(`test: `, test)

assert.strictEqual(s.name, "test")
