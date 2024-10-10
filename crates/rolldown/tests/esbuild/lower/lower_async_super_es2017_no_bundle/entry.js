class Derived extends Base {
	async test(key) {
		return [
			await super.foo,
			await super[key],
			await ([super.foo] = [0]),
			await ([super[key]] = [0]),

			await (super.foo = 1),
			await (super[key] = 1),
			await (super.foo += 2),
			await (super[key] += 2),

			await ++super.foo,
			await ++super[key],
			await super.foo++,
			await super[key]++,

			await super.foo.name,
			await super[key].name,
			await super.foo?.name,
			await super[key]?.name,

			await super.foo(1, 2),
			await super[key](1, 2),
			await super.foo?.(1, 2),
			await super[key]?.(1, 2),

			await (() => super.foo)(),
			await (() => super[key])(),
			await (() => super.foo())(),
			await (() => super[key]())(),

			await super.foo` + "``" + `,
			await super[key]` + "``" + `,
		]
	}
}

// This covers a bug that caused a compiler crash
let fn = async () => class extends Base {
	a = super.a
	b = () => super.b
	c() { return super.c }
	d() { return () => super.d }
}

// This covers a bug that generated bad code
class Derived2 extends Base {
	async a() { return class { [super.foo] = 123 } }
	b = async () => class { [super.foo] = 123 }
}

// This covers putting the generated temporary variable inside the loop
for (let i = 0; i < 3; i++) {
	objs.push({
		__proto__: {
			foo() { return i },
		},
		async bar() { return super.foo() },
	})
}