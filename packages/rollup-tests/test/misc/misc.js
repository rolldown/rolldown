const assert = require('node:assert');
const rollup = require('../../dist/rollup');
const { loader } = require('../utils.js');

describe('misc', () => {
	it('avoids modification of options or their properties', () => {
		const { freeze } = Object;
		return rollup.rollup(
			freeze({
				input: 'input',
				external: freeze([]),
				plugins: freeze([
					{
						name: 'loader',
						resolveId: freeze(() => 'input'),
						load: freeze(() => `export default 0;`)
					}
				]),
				acornInjectPlugins: freeze([]),
				acorn: freeze({}),
				treeshake: freeze({})
			})
		);
	});

	it('warns if node builtins are unresolved in a non-CJS, non-ES bundle (#1051)', () => {
		const warnings = [];

		return rollup
			.rollup({
				input: 'input',
				plugins: [
					loader({
						input: `import { format } from 'util';\nexport default format( 'this is a %s', 'formatted string' );`
					})
				],
				onwarn: warning => warnings.push(warning)
			})
			.then(bundle =>
				bundle.generate({
					format: 'iife',
					name: 'myBundle'
				})
			)
			.then(() => {
				const relevantWarnings = warnings.filter(
					warning => warning.code === 'MISSING_NODE_BUILTINS'
				);
				assert.equal(relevantWarnings.length, 1);
				assert.equal(
					relevantWarnings[0].message,
					`Creating a browser bundle that depends on Node.js built-in modules ("util"). You might need to include https://github.com/FredKSchott/rollup-plugin-polyfill-node`
				);
			});
	});

	it('warns when a global module name is guessed in a UMD bundle (#2358)', () => {
		const warnings = [];

		return rollup
			.rollup({
				input: 'input',
				external: ['lodash'],
				plugins: [
					loader({
						input: `import * as _ from 'lodash'; console.log(_);`
					})
				],
				onwarn: warning => warnings.push(warning)
			})
			.then(bundle =>
				bundle.generate({
					format: 'umd',
					globals: [],
					name: 'myBundle'
				})
			)
			.then(() => {
				delete warnings[0].toString;
				assert.deepEqual(warnings, [
					{
						code: 'MISSING_GLOBAL_NAME',
						id: 'lodash',
						message:
							'No name was provided for external module "lodash" in "output.globals" – guessing "_".',
						names: ['_'],
						url: 'https://rollupjs.org/configuration-options/#output-globals'
					}
				]);
			});
	});

	it('sorts chunks in the output', () => {
		const warnings = [];

		return rollup
			.rollup({
				input: ['main1', 'main2'],
				plugins: [
					loader({
						main1: 'import "dep";console.log("main1");',
						main2: 'import "dep";console.log("main2");',
						dep: 'console.log("dep");import("dyndep");',
						dyndep: 'console.log("dyndep");'
					})
				],
				onwarn: warning => warnings.push(warning)
			})
			.then(bundle => bundle.generate({ format: 'es' }))
			.then(({ output }) => {
				assert.equal(warnings.length, 0);
				assert.deepEqual(
					output.map(({ fileName }) => fileName),
					['main1.js', 'main2.js', 'dep-9394ae8f.js', 'dyndep-d5d54b59.js']
				);
			});
	});

	it('ignores falsy plugins', () =>
		rollup.rollup({
			input: 'x',
			plugins: [loader({ x: `console.log( 42 );` }), null, false, undefined]
		}));

	it('handles different import paths for different outputs', () =>
		rollup
			.rollup({
				input: 'x',
				external: ['the-answer'],
				plugins: [loader({ x: `import 'the-answer'` })]
			})
			.then(bundle =>
				Promise.all([
					bundle
						.generate({ format: 'es' })
						.then(generated =>
							assert.equal(generated.output[0].code, "import 'the-answer';\n", 'no render path 1')
						),
					bundle
						.generate({ format: 'es', paths: id => `//unpkg.com/${id}@?module` })
						.then(generated =>
							assert.equal(
								generated.output[0].code,
								"import '//unpkg.com/the-answer@?module';\n",
								'with render path'
							)
						),
					bundle
						.generate({ format: 'es' })
						.then(generated =>
							assert.equal(generated.output[0].code, "import 'the-answer';\n", 'no render path 2')
						)
				])
			));

	it('allows passing the same object to `rollup` and `generate`', () => {
		const options = {
			input: 'input',
			onwarn(warning, handler) {
				if (warning.code !== 'INPUT_HOOK_IN_OUTPUT_PLUGIN') {
					handler(warning);
				}
			},
			plugins: [
				loader({
					input: 'export default 42;'
				})
			],
			output: {
				format: 'es'
			}
		};

		return rollup
			.rollup(options)
			.then(bundle => bundle.generate(options))
			.then(output =>
				assert.strictEqual(
					output.output[0].code,
					'var input = 42;\n\nexport { input as default };\n'
				)
			);
	});

	it('consistently handles comments when using the cache', async () => {
		const FILES = {
			main: `import value from "other";
console.log(value);
/*#__PURE__*/console.log('removed');`,
			other: `var x = "foo";
export default x;
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoib3RoZXIuanMiLCJzb3VyY2VSb290IjoiIiwic291cmNlcyI6WyJvdGhlci50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiQUFBQSxJQUFNLENBQUMsR0FBVyxLQUFLLENBQUM7QUFDeEIsZUFBZSxDQUFDLENBQUMifQ==`
		};
		const EXPECTED_OUTPUT = `var x = "foo";

console.log(x);
`;
		const firstBundle = await rollup.rollup({
			input: 'main',
			plugins: [loader(FILES)]
		});
		assert.strictEqual(
			(await firstBundle.generate({ format: 'es' })).output[0].code,
			EXPECTED_OUTPUT,
			'first'
		);
		const secondBundle = await rollup.rollup({
			cache: firstBundle.cache,
			input: 'main',
			plugins: [loader(FILES)]
		});
		assert.strictEqual(
			(await secondBundle.generate({ format: 'es' })).output[0].code,
			EXPECTED_OUTPUT,
			'second'
		);
	});

	it('handles imports from chunks with names that correspond to parent directory names of other chunks', async () => {
		const bundle = await rollup.rollup({
			input: {
				'base/main': 'main.js',
				'base/main/feature': 'feature.js',
				'base/main/feature/sub': 'subfeature.js',
				'base/main/feature/sub/sub': 'subsubfeature.js'
			},
			plugins: [
				loader({
					'main.js': 'export function fn () { return "main"; } console.log(fn());',
					'feature.js': 'import { fn } from "main.js"; console.log(fn() + " feature");',
					'subfeature.js': 'import { fn } from "main.js"; console.log(fn() + " subfeature");',
					'subsubfeature.js': 'import { fn } from "main.js"; console.log(fn() + " subsubfeature");'
				})
			]
		});
		const {
			output: [main, feature, subfeature, subsubfeature]
		} = await bundle.generate({
			entryFileNames: `[name]`,
			chunkFileNames: `[name]`,
			format: 'es'
		});
		assert.strictEqual(main.fileName, 'base/main');
		assert.strictEqual(feature.fileName, 'base/main/feature');
		assert.ok(feature.code.startsWith("import { fn } from '../main'"));
		assert.strictEqual(subfeature.fileName, 'base/main/feature/sub');
		assert.ok(subfeature.code.startsWith("import { fn } from '../../main'"));
		assert.strictEqual(subsubfeature.fileName, 'base/main/feature/sub/sub');
		assert.ok(subsubfeature.code.startsWith("import { fn } from '../../../main'"));
	});

	it('throws the proper error on max call stack exception', async () => {
		const count = 10_000;
		let source = '';
		for (let index = 0; index < count; index++) {
			source += `if (foo) {`;
		}
		for (let index = 0; index < count; index++) {
			source += '}';
		}
		try {
			await rollup.rollup({
				input: {
					input: 'input'
				},
				plugins: [
					loader({
						input: source
					})
				]
			});
		} catch (error) {
			assert.notDeepStrictEqual(error.message, 'Maximum call stack size exceeded');
			assert.strictEqual(error.name, 'RollupError');
		}
	});

	it('supports rendering es after rendering iife with inlined dynamic imports', async () => {
		const bundle = await rollup.rollup({
			input: 'main.js',
			plugins: [
				loader({
					'main.js': "import('other.js');",
					'other.js': "export const foo = 'bar';"
				})
			]
		});
		await bundle.generate({ format: 'iife', inlineDynamicImports: true });
		await bundle.generate({ format: 'es', exports: 'auto' });
	});
});
