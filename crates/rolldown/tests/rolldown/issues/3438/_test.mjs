import assert from 'node:assert'
import main from './dist/main.js'

assert.equal(main[0], 'repro1_clear')
assert.equal(main[1], 'repro2_clear')
assert.equal(main[2], 'repro2_clear$1')
assert.equal(main[3], 'repro3_clear')
assert.equal(main[4], 'repro3_clear$1')
assert.equal(main[5], 'repro3_clear$2')
