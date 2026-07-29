// `require.resolve` survives bundling, so Node resolves it against the directory
// the generated config lives in. It must stay next to this file.
declare const require: { resolve: (id: string) => string };

export default { input: require.resolve('./runtime-required-entry.js') };
