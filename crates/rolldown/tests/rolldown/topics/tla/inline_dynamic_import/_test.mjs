import assert from "node:assert";
import run from './dist/main.js'

assert.strictEqual(await run(), '/demo/');