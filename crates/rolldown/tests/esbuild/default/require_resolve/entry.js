console.log(require.resolve)
console.log(require.resolve())
console.log(require.resolve(foo))
console.log(require.resolve('a', 'b'))
console.log(require.resolve('./present-file'))
console.log(require.resolve('./missing-file'))
console.log(require.resolve('./external-file'))
console.log(require.resolve('missing-pkg'))
console.log(require.resolve('external-pkg'))
console.log(require.resolve('@scope/missing-pkg'))
console.log(require.resolve('@scope/external-pkg'))
try {
	console.log(require.resolve('inside-try'))
} catch (e) {
}
if (false) {
	console.log(require.resolve('dead-code'))
}
console.log(false ? require.resolve('dead-if') : 0)
console.log(true ? 0 : require.resolve('dead-if'))
console.log(false && require.resolve('dead-and'))
console.log(true || require.resolve('dead-or'))
console.log(true ?? require.resolve('dead-nullish'))
