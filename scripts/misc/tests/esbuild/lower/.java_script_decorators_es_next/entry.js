@x.y()
@(new y.x)
export default class Foo {
	@x @y mUndef
	@x @y mDef = 1
	@x @y method() { return new Foo }
	@x @y static sUndef
	@x @y static sDef = new Foo
	@x @y static sMethod() { return new Foo }
}