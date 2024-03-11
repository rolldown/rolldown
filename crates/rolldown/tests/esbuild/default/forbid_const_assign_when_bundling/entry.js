let err
try {
  const x = 1
  x = 2
} catch (e) {
  err = e
}
assert(typeof err !== 'undefined')

function foo() {
  let err
  try {
    const y = 1
    y = 2
  } catch (e) {
    err = e
  }
  assert(typeof err !== 'undefined')
}

foo()
