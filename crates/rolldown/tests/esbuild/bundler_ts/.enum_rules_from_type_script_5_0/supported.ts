// From https://github.com/microsoft/TypeScript/pull/50528:
// "An expression is considered a constant expression if it is
const enum Foo {
	// a number or string literal,
	X0 = 123,
	X1 = 'x',

	// a unary +, -, or ~ applied to a numeric constant expression,
	X2 = +1,
	X3 = -2,
	X4 = ~3,

	// a binary +, -, *, /, %, **, <<, >>, >>>, |, &, ^ applied to two numeric constant expressions,
	X5 = 1 + 2,
	X6 = 1 - 2,
	X7 = 2 * 3,
	X8 = 1 / 2,
	X9 = 3 % 2,
	X10 = 2 ** 3,
	X11 = 1 << 2,
	X12 = -9 >> 1,
	X13 = -9 >>> 1,
	X14 = 5 | 12,
	X15 = 5 & 12,
	X16 = 5 ^ 12,

	// a binary + applied to two constant expressions whereof at least one is a string,
	X17 = 'x' + 0,
	X18 = 0 + 'x',
	X19 = 'x' + 'y',
	X20 = '' + NaN,
	X21 = '' + Infinity,
	X22 = '' + -Infinity,
	X23 = '' + -0,

	// a template expression where each substitution expression is a constant expression,
	X24 = ` + "`A${0}B${'x'}C${1 + 3 - 4 / 2 * 5 ** 6}D`" + `,

	// a parenthesized constant expression,
	X25 = (321),

	// a dotted name (e.g. x.y.z) that references a const variable with a constant expression initializer and no type annotation,
	/* (we don't implement this one) */

	// a dotted name that references an enum member with an enum literal type, or
	X26 = X0,
	X27 = X0 + 'x',
	X28 = 'x' + X0,
	X29 = ` + "`a${X0}b`" + `,
	X30 = Foo.X0,
	X31 = Foo.X0 + 'x',
	X32 = 'x' + Foo.X0,
	X33 = ` + "`a${Foo.X0}b`" + `,

	// a dotted name indexed by a string literal (e.g. x.y["z"]) that references an enum member with an enum literal type."
	X34 = X1,
	X35 = X1 + 'y',
	X36 = 'y' + X1,
	X37 = ` + "`a${X1}b`" + `,
	X38 = Foo['X1'],
	X39 = Foo['X1'] + 'y',
	X40 = 'y' + Foo['X1'],
	X41 = ` + "`a${Foo['X1']}b`" + `,
}

console.log(
	// a number or string literal,
	Foo.X0,
	Foo.X1,

	// a unary +, -, or ~ applied to a numeric constant expression,
	Foo.X2,
	Foo.X3,
	Foo.X4,

	// a binary +, -, *, /, %, **, <<, >>, >>>, |, &, ^ applied to two numeric constant expressions,
	Foo.X5,
	Foo.X6,
	Foo.X7,
	Foo.X8,
	Foo.X9,
	Foo.X10,
	Foo.X11,
	Foo.X12,
	Foo.X13,
	Foo.X14,
	Foo.X15,
	Foo.X16,

	// a template expression where each substitution expression is a constant expression,
	Foo.X17,
	Foo.X18,
	Foo.X19,
	Foo.X20,
	Foo.X21,
	Foo.X22,
	Foo.X23,

	// a template expression where each substitution expression is a constant expression,
	Foo.X24,

	// a parenthesized constant expression,
	Foo.X25,

	// a dotted name that references an enum member with an enum literal type, or
	Foo.X26,
	Foo.X27,
	Foo.X28,
	Foo.X29,
	Foo.X30,
	Foo.X31,
	Foo.X32,
	Foo.X33,

	// a dotted name indexed by a string literal (e.g. x.y["z"]) that references an enum member with an enum literal type."
	Foo.X34,
	Foo.X35,
	Foo.X36,
	Foo.X37,
	Foo.X38,
	Foo.X39,
	Foo.X40,
	Foo.X41,
)