import a from 'extern-esm'
import b from './esm'
import c from 'data:application/json,2'
import d from './file.file'
import e from './copy.copy'
console.log(
	a,
	b,
	c,
	d,
	e,
	require('extern-cjs'),
	require('./cjs'),
	import('./dynamic'),
)
export let exported