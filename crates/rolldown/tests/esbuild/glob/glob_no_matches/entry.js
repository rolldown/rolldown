const ab = Math.random() < 0.5 ? 'a.js' : 'b.js'
console.log({
	concat: {
		require: require('./src/' + ab + '.json'),
		import: import('./src/' + ab + '.json'),
	},
	template: {
		require: require(`./src/${ab}.json`),
		import: import(`./src/${ab}.json`),
	},
})