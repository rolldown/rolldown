import value from './rolldown.webp'
import assert from 'node:assert'

assert(typeof value === 'string' && value.startsWith('data:image/webp;base64,'))
