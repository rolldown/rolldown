let foo = 1
class Foo {
	method1(@dec(foo) foo = 2) {}
	method2(@dec(() => foo) foo = 3) {}
}

class Bar {
	static x = class {
		static y = () => {
			let bar = 1
			@dec(bar)
			@dec(() => bar)
			class Baz {
				@dec(bar) method1() {}
				@dec(() => bar) method2() {}
				method3(@dec(() => bar) bar) {}
				method4(@dec(() => bar) bar) {}
			}
			return Baz
		}
	}
}