import assert from 'node:assert'
import { Foo } from './a.js'

// This should have __name injected because it's transformed from declaration to expression
class Bar {}

assert.strictEqual(Foo.name, "Foo")
assert.strictEqual(Bar.name, "Bar")
