import { x } from './enum-constants'
console.log([
	typeof x.b,
], [
	x.a + x.b,
], [
	x.a < x.b,
	x.a > x.b,
	x.a <= x.b,
	x.a >= x.b,
	x.a == x.b,
	x.a != x.b,
	x.a === x.b,
	x.a !== x.b,
], [
	x.a && x.b,
	x.a || x.b,
	x.a ?? x.b,
	x.a ? 'y' : 'n',
	!x.b ? 'y' : 'n',
])