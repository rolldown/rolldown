module.exports = {
	description: 'keep import assertions for dynamic imports',
	expectedWarnings: 'UNRESOLVED_IMPORT',
	options: {
		external: id => {
			if (id === 'unresolved') return null;
			return true;
		},
		plugins: [
			{
				resolveDynamicImport(specifier) {
					if (typeof specifier === 'object') {
						if (specifier.type === 'TemplateLiteral') {
							return "'resolvedString'";
						}
						if (specifier.type === 'BinaryExpression') {
							return { id: 'resolved-id', external: true };
						}
					} else if (specifier === 'external-resolved') {
						return { id: 'resolved-different', external: true };
					}
					return null;
				}
			}
		],
		output: { externalImportAssertions: false }
	}
};
