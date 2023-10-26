console.log([
	require,
	typeof require,
	require('./example.json'),
	require('./example.json', { type: 'json' }),
	require(window.SOME_PATH),
	module.require('./example.json'),
	module.require('./example.json', { type: 'json' }),
	module.require(window.SOME_PATH),
	require.resolve('some-path'),
	require.resolve(window.SOME_PATH),
	import('some-path'),
	import(window.SOME_PATH),
])