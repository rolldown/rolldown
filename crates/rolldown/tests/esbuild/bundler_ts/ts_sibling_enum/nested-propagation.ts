export namespace n { export enum a { b = 100 } }
export namespace n {
	export enum x {
		c = n.a.b,
		d = c * 2,
		e = x.d ** 2,
		f = x['e'] / 4,
	}
}
export namespace n {
	export enum x { g = f >> 4 }
	console.log(a.b, n.a.b, n['a']['b'], x.g, n.x.g, n['x']['g'])
}