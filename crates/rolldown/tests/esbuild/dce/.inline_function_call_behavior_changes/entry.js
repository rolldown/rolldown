function empty() {}
function id(x) { return x }

export let shouldBeWrapped = [
	id(foo.bar)(),
	id(foo[bar])(),
	id(foo?.bar)(),
	id(foo?.[bar])(),

	(empty(), foo.bar)(),
	(empty(), foo[bar])(),
	(empty(), foo?.bar)(),
	(empty(), foo?.[bar])(),

	id(eval)(),
	id(eval)?.(),
	(empty(), eval)(),
	(empty(), eval)?.(),

	id(foo.bar)` + "``" + `,
	id(foo[bar])` + "``" + `,
	id(foo?.bar)` + "``" + `,
	id(foo?.[bar])` + "``" + `,

	(empty(), foo.bar)` + "``" + `,
	(empty(), foo[bar])` + "``" + `,
	(empty(), foo?.bar)` + "``" + `,
	(empty(), foo?.[bar])` + "``" + `,

	delete id(foo),
	delete id(foo.bar),
	delete id(foo[bar]),
	delete id(foo?.bar),
	delete id(foo?.[bar]),

	delete (empty(), foo),
	delete (empty(), foo.bar),
	delete (empty(), foo[bar]),
	delete (empty(), foo?.bar),
	delete (empty(), foo?.[bar]),

	delete empty(),
]

export let shouldNotBeWrapped = [
	id(foo)(),
	(empty(), foo)(),

	id(foo)` + "``" + `,
	(empty(), foo)` + "``" + `,
]

export let shouldNotBeDoubleWrapped = [
	delete (empty(), foo(), bar()),
	delete id((foo(), bar())),
]