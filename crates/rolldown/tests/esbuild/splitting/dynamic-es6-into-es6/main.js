import assert from 'node:assert'


import("./foo.js").then(({bar}) => {
  assert.equal(bar, 123)
})
