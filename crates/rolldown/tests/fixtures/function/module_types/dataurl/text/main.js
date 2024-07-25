import assert from 'node:assert';
import value from './text.data'

assert(typeof value === 'string' && value.startsWith('data:text/plain;charset=utf-8,'))
