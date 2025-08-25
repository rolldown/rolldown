import assert from 'node:assert'
class Example {
  static {
    this.prop = new Example('bar')
    assert.strictEqual(Example.prop.foo, 'bar')
  }

  constructor(foo) {
    this.foo = foo;
  }
}
