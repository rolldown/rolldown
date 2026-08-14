import a from 'pkg'
import b from './file'
console.log(
	a,
	b,
	require('pkg2'),
	require('./file2'),
	import('./dynamic'),
)
export let exported