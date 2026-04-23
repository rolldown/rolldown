class Foo {
	static { console.log('a') }
	a = 1
	static { console.log('b') }
	constructor(public b1 = 2.1, public b2 = 2.2) {
	}
	static { console.log('c') }
	c = 3
}