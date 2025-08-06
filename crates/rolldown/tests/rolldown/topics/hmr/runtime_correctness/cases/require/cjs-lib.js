exports.foo  = 'foo'
this.bar = 'bar'

class Noop {
  static {
    this.baz = 'bar'
  }
}

console.log(Noop)

{
  const exports = {}
  this.qux = 'qux'
}