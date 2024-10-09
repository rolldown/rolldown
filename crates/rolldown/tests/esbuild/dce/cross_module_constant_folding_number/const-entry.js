import { a, b } from './const-constants'
console.log([
	+b,
	-b,
	~b,
	!b,
	typeof b,
], [
	a + b,
	a - b,
	a * b,
	a / b,
	a % b,
	a ** b,
], [
	a < b,
	a > b,
	a <= b,
	a >= b,
	a == b,
	a != b,
	a === b,
	a !== b,
], [
	b << 1,
	b >> 1,
	b >>> 1,
], [
	a & b,
	a | b,
	a ^ b,
], [
	a && b,
	a || b,
	a ?? b,
	a ? 'y' : 'n',
	!b ? 'y' : 'n',
])