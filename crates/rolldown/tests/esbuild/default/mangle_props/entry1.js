export function shouldMangle() {
	let foo = {
		bar_: 0,
		baz_() {},
	};
	let { bar_ } = foo;
	({ bar_ } = foo);
	class foo_ {
		bar_ = 0
		baz_() {}
		static bar_ = 0
		static baz_() {}
	}
	return { bar_, foo_ }
}

export function shouldNotMangle() {
	let foo = {
		'bar_': 0,
		'baz_'() {},
	};
	let { 'bar_': bar_ } = foo;
	({ 'bar_': bar_ } = foo);
	class foo_ {
		'bar_' = 0
		'baz_'() {}
		static 'bar_' = 0
		static 'baz_'() {}
	}
	return { 'bar_': bar_, 'foo_': foo_ }
}