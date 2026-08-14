import assert from 'node:assert/strict';
import { defineConfig } from 'rolldown';

// Every option below is a real rolldown option reached via CLI dot-notation.
// cac only camelCases top-level option names, leaving nested keys in kebab-case;
// rolldown must camelCase them too (#9932) so they match the JS option names.
//
// This config function receives the parsed CLI args and asserts the converted
// shape. Because the CLI validates options against the schema *before* calling
// this function (strict objects reject unknown fields), and because the CLI
// options are then merged into the real build, reaching these assertions with a
// clean exit also proves each key actually exists in rolldown's options.
export default defineConfig((args) => {
  // output options
  assert.deepStrictEqual(args.generatedCode, {
    symbols: true,
    profilerNames: true,
  });
  assert.deepStrictEqual(args.advancedChunks, { minShareCount: 2 });

  // input options (incl. deeply nested)
  assert.deepStrictEqual(args.transform, {
    assumptions: { objectRestNoSymbols: true },
    typescript: { onlyRemoveTypeImports: true },
  });
  assert.deepStrictEqual(args.optimization, { inlineConst: true });
  assert.deepStrictEqual(args.checks, { circularDependency: true });

  return { input: './index.js' };
});
