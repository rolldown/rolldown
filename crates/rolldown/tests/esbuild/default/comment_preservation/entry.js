console.log(
	import(/* before */ foo),
	import(/* before */ 'foo'),
	import(foo /* after */),
	import('foo' /* after */),
)

console.log(
	import('foo', /* before */ { assert: { type: 'json' } }),
	import('foo', { /* before */ assert: { type: 'json' } }),
	import('foo', { assert: /* before */ { type: 'json' } }),
	import('foo', { assert: { /* before */ type: 'json' } }),
	import('foo', { assert: { type: /* before */ 'json' } }),
	import('foo', { assert: { type: 'json' /* before */ } }),
	import('foo', { assert: { type: 'json' } /* before */ }),
	import('foo', { assert: { type: 'json' } } /* before */),
)

console.log(
	require(/* before */ foo),
	require(/* before */ 'foo'),
	require(foo /* after */),
	require('foo' /* after */),
)

console.log(
	require.resolve(/* before */ foo),
	require.resolve(/* before */ 'foo'),
	require.resolve(foo /* after */),
	require.resolve('foo' /* after */),
)

let [/* foo */] = [/* bar */];
let [
	// foo
] = [
	// bar
];
let [/*before*/ ...s] = [/*before*/ ...s]
let [... /*before*/ s2] = [... /*before*/ s2]

let { /* foo */ } = { /* bar */ };
let {
	// foo
} = {
	// bar
};
let { /*before*/ ...s3 } = { /*before*/ ...s3 }
let { ... /*before*/ s4 } = { ... /*before*/ s4 }

let [/* before */ x] = [/* before */ x];
let [/* before */ x2 /* after */] = [/* before */ x2 /* after */];
let [
	// before
	x3
	// after
] = [
	// before
	x3
	// after
];

let { /* before */ y } = { /* before */ y };
let { /* before */ y2 /* after */ } = { /* before */ y2 /* after */ };
let {
	// before
	y3
	// after
} = {
	// before
	y3
	// after
};
let { /* before */ [y4]: y4 } = { /* before */ [y4]: y4 };
let { [/* before */ y5]: y5 } = { [/* before */ y5]: y5 };
let { [y6 /* after */]: y6 } = { [y6 /* after */]: y6 };

foo[/* before */ x] = foo[/* before */ x]
foo[x /* after */] = foo[x /* after */]

console.log(
	// before
	foo,
	/* comment before */
	bar,
	// comment after
)

console.log([
	// before
	foo,
	/* comment before */
	bar,
	// comment after
])

console.log({
	// before
	foo,
	/* comment before */
	bar,
	// comment after
})

console.log(class {
	// before
	foo
	/* comment before */
	bar
	// comment after
})

console.log(
	() => { return /* foo */ null },
	() => { throw /* foo */ null },
	() => { return (/* foo */ null) + 1 },
	() => { throw (/* foo */ null) + 1 },
	() => {
		return (// foo
			null) + 1
	},
	() => {
		throw (// foo
			null) + 1
	},
)

console.log(
	/*a*/ a ? /*b*/ b : /*c*/ c,
	a /*a*/ ? b /*b*/ : c /*c*/,
)

for (/*foo*/a;;);
for (;/*foo*/a;);
for (;;/*foo*/a);

for (/*foo*/a in b);
for (a in /*foo*/b);

for (/*foo*/a of b);
for (a of /*foo*/b);

if (/*foo*/a);
with (/*foo*/a);
while (/*foo*/a);
do {} while (/*foo*/a);
switch (/*foo*/a) {}