import assert from "node:assert"

class Foo {
	static foo = new Foo
}
let foo = Foo.foo
let foo2 = Foo.foo
let foo3 = Foo.foo
// assert(foo instanceof Foo)
// export class Bar {}
// export let bar = 123
