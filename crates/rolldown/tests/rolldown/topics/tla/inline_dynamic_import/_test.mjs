import assert from "node:assert";
import { foo } from './dist/main.js'

assert.strictEqual(foo, 'foo');