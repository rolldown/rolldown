import { defineConfig } from 'vite-plus';

export default defineConfig({
  staged: {
    '*': ['vp check --fix'],
  },
  lint: {
    options: {
      denyWarnings: true,
      // typeAware: true,
      // typeCheck: true
    },
    plugins: ['import', 'jsdoc', 'unicorn', 'typescript', 'oxc'],
    jsPlugins: ['./scripts/lint/index.ts'],
    ignorePatterns: [
      'crates/**',
      'packages/rollup-tests/**',
      'packages/rolldown/tests/fixtures/**',
      '!packages/rolldown/tests/fixtures/**/_config.ts',
      'packages/rolldown/tests/stability/**',
      'packages/rolldown/tests/magic-string/*.test.ts',
      'packages/rolldown/src/binding.*',
      'packages/test-dev-server/tests/fixtures/**',
      'packages/vite-tests/repo/**',
      'rollup/**',
      'test262/**',
    ],
    rules: {
      'import/named': 'error',
      'import/namespace': [
        'error',
        {
          allowComputed: true,
        },
      ],
      'jsdoc/check-tag-names': [
        'error',
        {
          definedTags: [
            'category',
            'include',
            'experimental',
            'inline',
            'hidden',
            'group',
            'typeParam',
            'inlineType',
          ],
        },
      ],
      'no-unused-expressions': [
        'warn',
        {
          allowShortCircuit: true,
          allowTaggedTemplates: true,
        },
      ],
      'no-unused-vars': [
        'warn',
        {
          varsIgnorePattern: '^_',
          argsIgnorePattern: '^_',
        },
      ],
      'unicorn/prefer-node-protocol': 'error',
      'typescript/no-base-to-string': 'allow',
      'typescript/no-floating-promises': 'allow',
      'typescript/consistent-type-imports': 'error',
      'typescript/restrict-template-expressions': 'allow',
    },
    overrides: [
      {
        files: ['**/packages/rolldown/src/**'],
        rules: {
          'no-console': [
            'warn',
            {
              allow: ['warn', 'error', 'debug', 'info'],
            },
          ],
        },
      },
      {
        files: ['**/packages/rolldown/tests/fixtures/**/_config.ts'],
        rules: {
          'rolldown-custom/ban-expect-assertions': 'error',
        },
      },
    ],
  },
  fmt: {
    singleQuote: true,
    ignorePatterns: [
      '**/.pnp.cjs',
      'rollup',
      'test262',
      'CHANGELOG-*.md',
      'CHANGELOG.md',
      'crates/rolldown/src/runtime',
      'crates/rolldown/tests/esbuild',
      '!crates/rolldown/tests/esbuild/**/_config.json',
      'crates/rolldown/tests/rolldown/errors',
      '!crates/rolldown/tests/rolldown/errors/**/_config.json',
      'crates/rolldown/tests/rolldown/topics/hmr/generate_patch_error/**/*.js',
      'crates/rolldown/tests/rolldown/topics/hmr/error_recovery/**/*.js',
      'crates/rolldown/tests/rolldown/topics/deconflict/.reserved_names/*.js',
      'crates/rolldown_plugin_hmr/src/runtime',
      'crates/rolldown_testing/_config.schema.json',
      'packages/rolldown/src/binding.cjs',
      'packages/rolldown/src/binding.d.cts',
      'packages/rolldown/src/browser.js',
      'packages/rolldown/src/rolldown-binding.wasi-browser.js',
      'packages/rolldown/src/rolldown-binding.wasi.cjs',
      'packages/rolldown/src/wasi-worker-browser.mjs',
      'packages/rolldown/src/wasi-worker.mjs',
      'packages/rolldown/tests/fixtures/misc/error/diagnostics/**/*.js',
      'packages/rolldown/tests/stability/issue-3453/src',
      'packages/rollup-tests',
      'packages/rollup/test',
      'packages/vite-tests',
      'scripts/snap-diff/stats',
      'scripts/snap-diff/summary',
      'scripts/src/esbuild-tests/snap-diff/**/*.md',
      'packages/debug/src/generated/**',
    ],
  },
});
