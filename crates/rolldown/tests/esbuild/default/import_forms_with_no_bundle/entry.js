import 'foo'
import {} from 'foo'
import * as ns from 'foo'
import {a, b as c} from 'foo'
import def from 'foo'
import def2, * as ns2 from 'foo'
import def3, {a2, b as c3} from 'foo'
const imp = [
	import('foo'),
	function nested() { return import('foo') },
]
console.log(ns, a, c, def, def2, ns2, def3, a2, c3, imp)