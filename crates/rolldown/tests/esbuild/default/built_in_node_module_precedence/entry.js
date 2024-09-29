console.log([
	// These are node core modules
	require('fs'),
	require('fs/promises'),
	require('node:foo'),

	// These are not node core modules
	require('fs/abc'),
	require('fs/'),
])