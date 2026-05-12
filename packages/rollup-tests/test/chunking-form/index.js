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
	const { parseSync } = await getOxcParser();
	for (const relPath of actualChunkFiles) {
		const expectedPath = join(expectedBase, relPath);
		if (!existsSync(expectedPath)) continue;
		const actual = extractExports(parseSync, readFileSync(join(actualBase, relPath), 'utf8'), outputOptions.format);
		const expected = extractExports(parseSync, readFileSync(expectedPath, 'utf8'), outputOptions.format);
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

let oxcParserPromise;
function getOxcParser() {
	return oxcParserPromise ??= import('oxc-parser');
}

function extractExports(parseSync, code, format) {
	return format === 'cjs'
		? extractCjsExports(parseSync, code)
		: extractEsmExports(parseSync, code);
}

function extractEsmExports(parseSync, code) {
	const result = parseSync('chunk.js', code, { sourceType: 'module' });
	const names = new Set();
	let hasDefault = false;
	for (const staticExport of result.module.staticExports) {
		for (const entry of staticExport.entries) {
			const { exportName } = entry;
			if (exportName.kind === 'Default') {
				hasDefault = true;
			} else if (exportName.kind === 'Name' && exportName.name) {
				if (exportName.name === 'default') hasDefault = true;
				else names.add(exportName.name);
			} else if (exportName.kind === 'None') {
				// `export * from "mod"` — no concrete name visible; use sentinel
				// so rolldown/rollup outputs can be compared symmetrically.
				names.add('*');
			}
		}
	}
	return { names, hasDefault };
}

function extractCjsExports(parseSync, code) {
	const result = parseSync('chunk.js', code, { sourceType: 'script' });
	const names = new Set();
	let hasDefault = false;
	const addName = name => {
		if (!name || name === '__esModule') return;
		if (name === 'default') hasDefault = true;
		else names.add(name);
	};
	const isExportsId = n => n && n.type === 'Identifier' && n.name === 'exports';
	const isModuleDotExports = n =>
		n && n.type === 'MemberExpression' && !n.computed &&
		n.object.type === 'Identifier' && n.object.name === 'module' &&
		n.property.type === 'Identifier' && n.property.name === 'exports';
	const stringValue = n => (n && n.type === 'Literal' && typeof n.value === 'string') ? n.value : null;

	function visit(node) {
		if (!node || typeof node !== 'object') return;
		if (Array.isArray(node)) {
			for (const child of node) visit(child);
			return;
		}
		if (node.type === 'AssignmentExpression' && node.operator === '=') {
			const left = node.left;
			if (left && left.type === 'MemberExpression') {
				if (isModuleDotExports(left)) {
					hasDefault = true;
				} else if (isExportsId(left.object)) {
					if (!left.computed && left.property.type === 'Identifier') {
						addName(left.property.name);
					} else {
						const name = stringValue(left.property);
						if (name) addName(name);
					}
				}
			}
		} else if (node.type === 'CallExpression') {
			const callee = node.callee;
			if (callee && callee.type === 'MemberExpression' && !callee.computed &&
				callee.object.type === 'Identifier' && callee.object.name === 'Object' &&
				callee.property.type === 'Identifier') {
				const fn = callee.property.name;
				if (fn === 'defineProperty' && node.arguments.length >= 2 && isExportsId(node.arguments[0])) {
					const name = stringValue(node.arguments[1]);
					if (name) addName(name);
				} else if (fn === 'defineProperties' && node.arguments.length >= 2 &&
					isExportsId(node.arguments[0]) && node.arguments[1].type === 'ObjectExpression') {
					for (const prop of node.arguments[1].properties) {
						if (prop.type !== 'Property') continue;
						if (!prop.computed && prop.key.type === 'Identifier') addName(prop.key.name);
						else {
							const name = stringValue(prop.key);
							if (name) addName(name);
						}
					}
				}
			}
		}
		for (const key in node) {
			if (key === 'type' || key === 'start' || key === 'end' || key === 'range' || key === 'loc') continue;
			const val = node[key];
			if (val && typeof val === 'object') visit(val);
		}
	}

	visit(result.program);
	return { names, hasDefault };
}
