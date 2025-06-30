// @ts-nocheck
import assert from 'node:assert'
import { myName } from './dist/main'

assert.strictEqual(myName, 'MyClass', 'Should return the correct name for MyClass after renaming')
