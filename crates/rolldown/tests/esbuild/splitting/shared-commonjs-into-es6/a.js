import assert from "node:assert"
import { foo } from "./shared.js"
assert.equal(foo, 123)
