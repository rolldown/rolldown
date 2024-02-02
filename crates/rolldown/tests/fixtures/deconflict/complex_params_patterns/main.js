import assert from 'assert'
import {
  a as a2,
  b as b2,
  c as c2,
  d as d2,
  e as e2,
} from './names.js'

export const [a, b, c, d, e] = ['a', 'b', 'c', 'd', 'e']

function foo(a$1, { b$1 }, { c$1 = 'c$1' }, { d$1 } = { d$1: 'd$1' }, { e$1 = '' } = { e$1: 'e$1' }) {
  return {
    main: [a, b, c, d, e],
    names: [a2, b2, c2, d2, e2],
    params: [a$1, b$1, c$1, d$1, e$1],
  }
}

const ret = foo('a$1', { b$1: 'b$1' }, {}, undefined, undefined)
assert.deepStrictEqual(ret.main, ['a', 'b', 'c', 'd', 'e'])
assert.deepStrictEqual(ret.names, ['a2', 'b2', 'c2', 'd2', 'e2'])
assert.deepStrictEqual(ret.params, ['a$1', 'b$1', 'c$1', 'd$1', 'e$1'])
