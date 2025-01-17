// @ts-nocheck
import assert from 'node:assert'
import { name, json } from './dist/main'

assert(name === '@test-fixture/named-exports')
assert(name === json.name)
assert(json.const === true)
