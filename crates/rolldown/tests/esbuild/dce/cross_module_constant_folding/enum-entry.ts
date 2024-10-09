import { x } from './enum-constants'
console.log([
	+x.b,
	-x.b,
	~x.b,
	!x.b,
	typeof x.b,
], [
	x.a + x.b,
	x.a - x.b,
	x.a * x.b,
	x.a / x.b,
	x.a % x.b,
	x.a ** x.b,
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
	x.b << 1,
	x.b >> 1,
	x.b >>> 1,
], [
	x.a & x.b,
	x.a | x.b,
	x.a ^ x.b,
], [
	x.a && x.b,
	x.a || x.b,
	x.a ?? x.b,
])