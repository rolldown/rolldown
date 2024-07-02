// Use const enums to force inline values
const enum Foo {
	NAN = 0 / 0,
	POS_INF = 1 / 0,
	NEG_INF = -1 / 0,
}

//! It's ok to use "NaN" and "Infinity" here
console.log(
	Foo.NAN,
	Foo.POS_INF,
	Foo.NEG_INF,
)
checkPrecedence(
	1 / Foo.NAN,
	1 / Foo.POS_INF,
	1 / Foo.NEG_INF,
)

//! We must not use "NaN" or "Infinity" inside "with"
with (x) {
	console.log(
		Foo.NAN,
		Foo.POS_INF,
		Foo.NEG_INF,
	)
	checkPrecedence(
		1 / Foo.NAN,
		1 / Foo.POS_INF,
		1 / Foo.NEG_INF,
	)
}