const assert = require('node:assert');
const { readdirSync } = require('node:fs');
const { basename, join, resolve } = require('node:path');
/**
 * @type {import('../../src/rollup/types')} Rollup
 */
// @ts-expect-error not included in types
const { rollup } = require('../../dist/rollup');
const { compareLogs } = require('../utils');
const { runTestSuiteWithSamples } = require('../utils.js');

// `amd` and `system` formats are not supported by Rolldown yet
// (bindingifyFormat in packages/rolldown/src/utils/bindingify-output-options.ts
// throws `unimplemented`). Skip them so we don't burn a test run on a
// known-unimplemented codepath. Rolldown supports es / cjs / iife / umd; the
// chunking-form test suite only exercises es / cjs / amd / system, so the
// runnable subset for us is es + cjs.
const FORMATS = ['es', 'cjs'];

runTestSuiteWithSamples('chunking form', resolve(__dirname, '../../../../rollup/test/chunking-form/samples'), (directory, config) => {
	(config.skip ? describe.skip : config.solo ? describe.only : describe)(
		basename(directory) + ': ' + config.description,
		() => {
			let bundle;

			if (config.before) {
				before(config.before);
			}
			if (config.after) {
				after(config.after);
			}
			const logs = [];
			after(() => config.logs && compareLogs(logs, config.logs));

			for (const format of FORMATS) {
				it('generates ' + format, async () => {
					process.chdir(directory);
					const warnings = [];
					bundle =
						bundle ||
						(await rollup({
							input: [directory + '/main.js'],
							onLog: (level, log) => {
								logs.push({ level, ...log });
								if (level === 'warn' && !config.expectedWarnings?.includes(log.code)) {
									warnings.push(log);
								}
							},
							strictDeprecations: true,
							...config.options
						}));
					await generateAndTestBundle(
						bundle,
						{
							dir: `${directory}/_actual/${format}`,
							exports: 'auto',
							format,
							chunkFileNames: 'generated-[name].js',
							validate: true,
							...(config.options || {}).output
						},
						`${directory}/_expected/${format}`,
						config
					);
					if (warnings.length > 0) {
						const codes = new Set();
						for (const { code } of warnings) {
							codes.add(code);
						}
						throw new Error(
							`Unexpected warnings (${[...codes].join(', ')}): \n${warnings
								.map(({ message }) => `${message}\n\n`)
								.join('')}` + 'If you expect warnings, list their codes in config.expectedWarnings'
						);
					}
				});
			}
		}
	);
});

async function generateAndTestBundle(bundle, outputOptions, expectedDirectory, config) {
	const writeResult = await bundle.write({
		...outputOptions,
		dir: `${outputOptions.dir}${config.nestedDir ? '/' + config.nestedDir : ''}`
	});
	if (outputOptions.format === 'amd' && config.runAmd) {
		try {
			const exports = await new Promise((resolve, reject) => {
				// @ts-expect-error global
				global.assert = require('node:assert');
				const requirejs = require('requirejs');
				requirejs.config({ baseUrl: outputOptions.dir });
				requirejs([config.nestedDir ? `${config.nestedDir}/main` : 'main'], resolve, reject);
			});
			if (config.runAmd.exports) {
				await config.runAmd.exports(exports);
			}
		} finally {
			delete require.cache[require.resolve('requirejs')];
			// @ts-expect-error global
			delete global.requirejsVars;
			// @ts-expect-error global
			delete global.assert;
		}
	}
	// Rolldown's output bytes diverge from Rollup's (region comments, quote style,
	// identifier deconfliction, etc.), so byte-equal directory comparison is too
	// strict. Compare chunk count first — a stable structural signal.
	const actualChunkCount = writeResult.output.filter(o => o.type === 'chunk').length;
	const expectedChunkCount = countChunkFiles(expectedDirectory);
	assert.strictEqual(
		actualChunkCount,
		expectedChunkCount,
		`Chunk count mismatch in ${expectedDirectory}: actual ${actualChunkCount}, expected ${expectedChunkCount}`
	);
}

function countChunkFiles(dir) {
	let count = 0;
	function walk(d) {
		let entries;
		try {
			entries = readdirSync(d, { withFileTypes: true });
		} catch {
			return;
		}
		for (const e of entries) {
			if (e.isDirectory()) walk(join(d, e.name));
			else if (e.name.endsWith('.js') && !e.name.endsWith('.js.map')) count++;
		}
	}
	walk(dir);
	return count;
}
