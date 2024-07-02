export enum a { b = 100 }
export enum x {
	c = a.b,
	d = c * 2,
	e = x.d ** 2,
	f = x['e'] / 4,
}
export enum x { g = f >> 4 }
console.log(a.b, a['b'], x.g, x['g'])