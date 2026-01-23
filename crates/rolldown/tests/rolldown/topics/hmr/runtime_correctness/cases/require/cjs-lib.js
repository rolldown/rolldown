exports.foo  = 'foo'
this.bar = 'bar'

class Noop {
  static baz2 = (this.baz3 = 'wrong')
  static {
    this.baz = 'bar'
  }
}

console.log(Noop)

{
  const exports = {}
  this.qux = 'qux'
}

import 'trigger-dep'