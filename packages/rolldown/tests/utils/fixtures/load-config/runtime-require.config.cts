declare const require: { resolve: (id: string) => string };

export default { input: require.resolve('./native.config.mjs') };
