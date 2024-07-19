import assert from 'node:assert';
import value from './text.txt'

assert(typeof value === 'string' && value.startsWith('data:text/plain;charset=utf-8,'))
