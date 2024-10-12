@x?.[_ + 'y']()
@new y?.[_ + 'x']()
export default class Foo {
	@x @y [mUndef()]
	@x @y [mDef()] = 1
	@x @y [method()](@x0 @y0 arg0, @x1 @y1 arg1) { return new Foo }
	@x @y declare [mDecl()]
	@x @y abstract [mAbst()]

	// Side effect order must be preserved even for fields without decorators
	[xUndef()]
	[xDef()] = 2
	static [yUndef()]
	static [yDef()] = 3

	@x @y static [sUndef()]
	@x @y static [sDef()] = new Foo
	@x @y static [sMethod()](@x0 @y0 arg0, @x1 @y1 arg1) { return new Foo }
	@x @y static declare [mDecl()]
}