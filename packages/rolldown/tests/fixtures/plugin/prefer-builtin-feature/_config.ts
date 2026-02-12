import { stripAnsi } from 'consola/utils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let warnings: string[] = [];
export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'json',
      },
      {
        name: 'inject',
      },
    ],
    onwarn(warning, ctx) {
      warnings.push(stripAnsi(warning.message));
    },
  },
  afterTest() {
    expect(warnings).toMatchInlineSnapshot(`
      [
        "[PREFER_BUILTIN_FEATURE] Warning: The functionality provided by \`@rollup/plugin-json\` is already covered natively, maybe you could remove the plugin from your configuration.
        │ 
        │ Help: This diagnostic may be false positive, you could turn it off via \`checks.preferBuiltinFeature\`
      ",
        "[PREFER_BUILTIN_FEATURE] Warning: Rolldown supports \`inject\` natively. Please refer https://rolldown.rs/reference/ for more details. It is more performant than passing \`@rollup/plugin-inject\` to plugins option.
        │ 
        │ Help: This diagnostic may be false positive, you could turn it off via \`checks.preferBuiltinFeature\`
      ",
      ]
    `);
  },
});
