import assert from 'assert'
import mod from './a.json'


assert.deepStrictEqual(mod, {
  eval: true,
  arguments: false,
  valid: true,
  let: true,
  _let: 'result',
  globalThis: false,
})
