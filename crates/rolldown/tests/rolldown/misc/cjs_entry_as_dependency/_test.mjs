import assert from 'assert'
import main from './dist/main.mjs'
import main2 from './dist/main2.mjs'
assert(main === main2)
