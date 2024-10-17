class Derived extends Base {
	static test = key => {
		return [
			super.foo,
			super[key],
			([super.foo] = [0]),
			([super[key]] = [0]),

			(super.foo = 1),
			(super[key] = 1),
			(super.foo += 2),
			(super[key] += 2),

			++super.foo,
			++super[key],
			super.foo++,
			super[key]++,

			super.foo.name,
			super[key].name,
			super.foo?.name,
			super[key]?.name,

			super.foo(1, 2),
			super[key](1, 2),
			super.foo?.(1, 2),
			super[key]?.(1, 2),

			(() => super.foo)(),
			(() => super[key])(),
			(() => super.foo())(),
			(() => super[key]())(),

			super.foo` + "``" + `,
			super[key]` + "``" + `,
		]
	}
}