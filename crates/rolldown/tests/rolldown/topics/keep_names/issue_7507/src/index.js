import assert from 'node:assert'
import { test as barTest } from "./bar.js";
import test from "./foo.js";

assert.strictEqual(barTest.name, "test");
assert.strictEqual(test.name, "test");
