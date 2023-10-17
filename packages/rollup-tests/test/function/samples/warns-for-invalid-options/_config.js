module.exports = {
	description: 'warns for invalid options',
	options: {
		myInvalidInputOption: true,
		output: {
			myInvalidOutputOption: true
		}
	},
	warnings: [
		{
			code: 'UNKNOWN_OPTION',
			message:
				'Unknown input options: myInvalidInputOption. Allowed options: ' +
				require('../../../misc/optionList').input
		},
		{
			code: 'UNKNOWN_OPTION',
			message:
				'Unknown output options: myInvalidOutputOption. Allowed options: ' +
				require('../../../misc/optionList').output
		}
	]
};
