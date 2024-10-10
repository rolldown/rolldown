class Derived extends Base {
	static test = async (key) => {
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
	static a = super.a
	static b = () => super.b
	static c() { return super.c }
	static d() { return () => super.d }
}

// This covers a bug that generated bad code
class Derived2 extends Base {
	static async a() { return class { [super.foo] = 123 } }
	static b = async () => class { [super.foo] = 123 }
}