const path = require('node:path');
/**
 * @type {import('../../src/rollup/types')} Rollup
 */
// @ts-expect-error not included in types
const rollup = require('../../dist/rollup');
// @ts-expect-error not included in types
const { compareLogs, runTestSuiteWithSamples } = require('../utils.js');

// TODO more formats
// const FORMATS = ['amd', 'cjs', 'system', 'es', 'iife', 'umd'];
const FORMATS = ['es'];

runTestSuiteWithSamples(
	'sourcemaps',
	path.resolve(__dirname, '../../../../rollup/test/sourcemaps/samples'),
	/**
	 * @param {import('../types').TestConfigSourcemap} config
	 */
	(directory, config) => {
		(config.skip ? describe.skip : config.solo ? describe.only : describe)(
			path.basename(directory) + ': ' + config.description,
			() => {
				for (const format of config.formats || FORMATS) {
					it('generates ' + format, async () => {
						process.chdir(directory);
						const warnings = [];
						const inputOptions = {
							input: directory + '/main.js',
							onwarn: warning => warnings.push(warning),
							strictDeprecations: true,
							...config.options
						};
						let customOutputOptions = (config.options || {}).output || {};
						const outputOptions = {
							exports: 'auto',
							dir: directory + '/_actual/',
							entryFileNames: customOutputOptions.file ? path.basename(customOutputOptions.file) : 'bundle.' + format + '.js',
							// file: directory + '/_actual/bundle.' + format + '.js',
							format,
							sourcemap: true,
							...customOutputOptions
						};

						let bundle = await rollup.rollup(inputOptions);
						await generateAndTestBundle(bundle, outputOptions, config, format, warnings);
						// cache rebuild does not reemit warnings.
						if (config.warnings) {
							return;
						}
						// test cache noop rebuild
						bundle = await rollup.rollup({ cache: bundle, ...inputOptions });
						await generateAndTestBundle(bundle, outputOptions, config, format, warnings);
					});
				}
			}
		);
	}
);

async function generateAndTestBundle(bundle, outputOptions, config, format, warnings) {
	if (config.warnings) {
		compareLogs(warnings, config.warnings);
	} else if (warnings.length > 0) {
		throw new Error(`Unexpected warnings`);
	}

	const {
		output: [{ code, map, fileName, sourcemapFileName }]
	} = await bundle.write(outputOptions);
	await config.test(code, map, { fileName, sourcemapFileName, format });
}
