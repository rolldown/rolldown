import assert from 'assert'

// Top-level variable
const num = 0

// Arrow function with parameter shadowing the top-level variable
// The parameter `num` should NOT be renamed - it's in its own scope
const config = (num) => num

// Test that both work correctly
assert.equal(num, 0)
assert.equal(config(42), 42)

export { config, num }
