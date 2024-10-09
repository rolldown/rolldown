import { a, b } from './const-constants'
console.log([
	typeof b,
], [
	a + b,
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
	a && b,
	a || b,
	a ?? b,
	a ? 'y' : 'n',
	!b ? 'y' : 'n',
])