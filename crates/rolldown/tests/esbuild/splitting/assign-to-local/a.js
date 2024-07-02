import assert from "node:assert"
import {foo, setFoo} from "./shared.js"
setFoo(123)
assert.equal(foo, 123)
