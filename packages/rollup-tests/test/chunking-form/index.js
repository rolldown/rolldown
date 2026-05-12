const assert = require('node:assert');
const { existsSync, readdirSync, readFileSync } = require('node:fs');
const { basename, join, relative, resolve } = require('node:path');
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
	const actualChunkFiles = collectChunkFiles(`${outputOptions.dir}${config.nestedDir ? '/' + config.nestedDir : ''}`);
	const expectedChunkFiles = collectChunkFiles(`${expectedDirectory}${config.nestedDir ? '/' + config.nestedDir : ''}`);
	assert.strictEqual(
		actualChunkFiles.length,
		expectedChunkFiles.length,
		`Chunk count mismatch in ${expectedDirectory}: actual ${actualChunkFiles.length}, expected ${expectedChunkFiles.length}`
	);
	// Also compare export signatures of corresponding chunks. For each chunk,
	// the set of exported names plus the presence of a default export must
	// match exactly between rolldown and rollup.
	const actualBase = `${outputOptions.dir}${config.nestedDir ? '/' + config.nestedDir : ''}`;
	const expectedBase = `${expectedDirectory}${config.nestedDir ? '/' + config.nestedDir : ''}`;
	for (const relPath of actualChunkFiles) {
		const expectedPath = join(expectedBase, relPath);
		if (!existsSync(expectedPath)) continue;
		const actual = extractExports(readFileSync(join(actualBase, relPath), 'utf8'), outputOptions.format);
		const expected = extractExports(readFileSync(expectedPath, 'utf8'), outputOptions.format);
		assert.deepStrictEqual(
			[...actual.names].sort(),
			[...expected.names].sort(),
			`Chunk ${relPath} export names differ: actual ${JSON.stringify([...actual.names].sort())} vs expected ${JSON.stringify([...expected.names].sort())}`
		);
		assert.strictEqual(
			actual.hasDefault,
			expected.hasDefault,
			`Chunk ${relPath} default-export presence differs: actual ${actual.hasDefault} vs expected ${expected.hasDefault}`
		);
	}
}

function collectChunkFiles(dir) {
	const result = [];
	function walk(d) {
		let entries;
		try {
			entries = readdirSync(d, { withFileTypes: true });
		} catch {
			return;
		}
		for (const e of entries) {
			const p = join(d, e.name);
			if (e.isDirectory()) walk(p);
			else if (e.name.endsWith('.js') && !e.name.endsWith('.js.map')) {
				result.push(relative(dir, p).replace(/\\/g, '/'));
			}
		}
	}
	walk(dir);
	return result;
}

function stripComments(code) {
	return code
		.replace(/\/\*[\s\S]*?\*\//g, '')
		.replace(/\/\/[^\n]*/g, '');
}

function extractExports(code, format) {
	const stripped = stripComments(code);
	return format === 'cjs' ? extractCjsExports(stripped) : extractEsmExports(stripped);
}

function extractEsmExports(code) {
	const names = new Set();
	let hasDefault = false;
	const addName = name => {
		if (!name) return;
		if (name === 'default') hasDefault = true;
		else names.add(name);
	};
	// export { a, b as c, d as default }
	const reBlock = /export\s*\{([\s\S]*?)\}/g;
	let m;
	while ((m = reBlock.exec(code))) {
		for (const spec of m[1].split(',')) {
			const s = spec.trim();
			if (!s) continue;
			const parts = s.split(/\s+as\s+/);
			const exported = (parts[1] || parts[0]).trim();
			// strip optional quotes for string-literal exports (e.g., `export { x as "default" }`)
			const unquoted = exported.replace(/^["']|["']$/g, '');
			addName(unquoted);
		}
	}
	// export default ...
	if (/\bexport\s+default\b/.test(code)) hasDefault = true;
	// export const|let|var X = ...
	const reVar = /\bexport\s+(?:const|let|var)\s+(\w+)/g;
	while ((m = reVar.exec(code))) addName(m[1]);
	// export function|class|async function X
	const reFn = /\bexport\s+(?:async\s+)?(?:function\*?|class)\s+(\w+)/g;
	while ((m = reFn.exec(code))) addName(m[1]);
	// export * as ns from '...'
	const reNs = /\bexport\s+\*\s+as\s+(\w+)\s+from\b/g;
	while ((m = reNs.exec(code))) addName(m[1]);
	// export * from '...' (no name visible) → sentinel for symmetric comparison
	const reStar = /\bexport\s+\*\s+from\b/g;
	while ((m = reStar.exec(code))) names.add('*');
	return { names, hasDefault };
}

function extractCjsExports(code) {
	const names = new Set();
	let hasDefault = false;
	const addName = name => {
		if (!name || name === '__esModule') return;
		if (name === 'default') hasDefault = true;
		else names.add(name);
	};
	// module.exports = ...
	if (/(?:^|[^.\w$])module\.exports\s*=/.test(code)) hasDefault = true;
	// exports.X = ...
	const reDot = /(?:^|[^.\w$])exports\.(\w+)\s*=/g;
	let m;
	while ((m = reDot.exec(code))) addName(m[1]);
	// exports["X"] = ...
	const reBracket = /(?:^|[^.\w$])exports\[\s*(['"])([^'"]+)\1\s*\]\s*=/g;
	while ((m = reBracket.exec(code))) addName(m[2]);
	// Object.defineProperty(exports, "X", ...)
	const reDefine = /Object\.defineProperty\s*\(\s*exports\s*,\s*(['"])([^'"]+)\1/g;
	while ((m = reDefine.exec(code))) addName(m[2]);
	// Object.defineProperties(exports, { X: ..., Y: ... })
	const reDefineMulti = /Object\.defineProperties\s*\(\s*exports\s*,\s*\{([\s\S]*?)\}\s*\)/g;
	while ((m = reDefineMulti.exec(code))) {
		const propRe = /(?:^|[,{\s])(['"]?)(\w+)\1\s*:/g;
		let pm;
		while ((pm = propRe.exec(m[1]))) addName(pm[2]);
	}
	return { names, hasDefault };
}
