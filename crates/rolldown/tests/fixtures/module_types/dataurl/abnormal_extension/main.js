import value from './data.custom'
import assert from 'node:assert'

assert(typeof value === 'string' && value.startsWith('data:image/png;base64,'))
