import assert from 'node:assert';
import value from './data.custom'

assert(typeof value === 'string' && value.startsWith('data:application/octet-stream;base64,'));

