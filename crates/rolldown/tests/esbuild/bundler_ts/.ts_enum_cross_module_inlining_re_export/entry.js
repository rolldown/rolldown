import { a } from './re-export'
import { b } from './re-export-star'
import * as ns from './enums'
console.log([
	a.x,
	b.x,
	ns.c.x,
])