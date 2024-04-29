declare let x: any, y: any
@x.y()
@(new y.x)
export default class Foo {
	@x @y mUndef: any
	@x @y mDef = 1
	@x @y method() { return new Foo }
	@x @y declare mDecl: any
	@x @y accessor aUndef: any
	@x @y accessor aDef = 1

	@x @y static sUndef: any
	@x @y static sDef = new Foo
	@x @y static sMethod() { return new Foo }
	@x @y static declare sDecl: any
	@x @y static accessor asUndef: any
	@x @y static accessor asDef = 1

	@x @y #mUndef: any
	@x @y #mDef = 1
	@x @y #method() { return new Foo }
	@x @y accessor #aUndef: any
	@x @y accessor #aDef = 1

	@x @y static #sUndef: any
	@x @y static #sDef = 1
	@x @y static #sMethod() { return new Foo }
	@x @y static accessor #asUndef: any
	@x @y static accessor #asDef = 1
}