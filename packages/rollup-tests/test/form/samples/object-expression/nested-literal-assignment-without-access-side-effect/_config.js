module.exports = {
	description: 'treats mutating nested properties as side effects',
	options: {
		treeshake: { propertyReadSideEffects: false }
	}
};
