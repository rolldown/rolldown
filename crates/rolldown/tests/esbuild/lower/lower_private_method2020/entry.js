export class Foo {
	#field
	#method() {}
	baseline() {
		a().foo
		b().foo(x)
		c()?.foo(x)
		d().foo?.(x)
		e()?.foo?.(x)
	}
	privateField() {
		a().#field
		b().#field(x)
		c()?.#field(x)
		d().#field?.(x)
		e()?.#field?.(x)
		f()?.foo.#field(x).bar()
	}
	privateMethod() {
		a().#method
		b().#method(x)
		c()?.#method(x)
		d().#method?.(x)
		e()?.#method?.(x)
		f()?.foo.#method(x).bar()
	}
}