module.exports = {
	description: 'Support external namespace reexport',
	options: {
		external: ['highcharts'],
		output: {
			globals: { highcharts: 'highcharts' },
			name: 'myBundle'
		}
	}
};
