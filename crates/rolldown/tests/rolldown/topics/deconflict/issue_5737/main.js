import { foo as foo_ } from "./foo.js"
import assert from 'node:assert'

export const foo = () => foo_

assert.strictEqual(foo.name, 'foo')
