import assert from 'node:assert'
import { star } from './re-export'

export function foo() {
  return "foo"
}

export const bar = "bar"

// Prevents rewrite to specific properties
assert.strictEqual(star['foo'.replace('', '')], foo)
assert.strictEqual(star['bar'.replace('', '')], "bar")