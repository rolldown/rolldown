const ab = Math.random() < 0.5 ? 'a.ts' : 'b.ts'
console.log({
	concat: {
		require: require('./src/' + ab),
		import: import('./src/' + ab),
	},
	template: {
		require: require(`./src/${ab}`),
		import: import(`./src/${ab}`),
	},
})