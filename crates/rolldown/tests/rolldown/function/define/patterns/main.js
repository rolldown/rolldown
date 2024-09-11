import assert from 'node:assert'

const id = Id
const objProp = Obj.prop

assert.strictEqual(id, 'ok')
assert.strictEqual(objProp, 'ok')

// It should not inject shadowed variables.
;(function (Id, Obj) {
  assert.strictEqual(Id, undefined)
  assert.strictEqual(Obj.prop, undefined)
})(undefined, {});
