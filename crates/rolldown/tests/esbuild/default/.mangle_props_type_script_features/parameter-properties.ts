class Foo {
	constructor(
		public KEEP_FIELD: number,
		public MANGLE_FIELD_: number,
	) {
	}
}

let foo = new Foo
console.log(foo.KEEP_FIELD, foo.MANGLE_FIELD_)