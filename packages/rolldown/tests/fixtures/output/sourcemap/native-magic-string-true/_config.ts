import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  sequential: true,
  config: {
    input: ['main.js'],
    experimental: {
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'test',
        transform(code, id, meta) {
          let { magicString } = meta;
          if (magicString) {
            magicString.replace('test', 'result');
            return {
              code: magicString,
            };
          }
        },
      },
    ],
  },
  afterTest: function (output) {
    expect(output.output[0].map).toBeDefined();
    // The last segment maps `"res"` to `'res'` (column 20). A line-level map from the
    // MagicString would map it to column 17, where the text after `test` starts.
    expect(output.output[0].map!.toString()).toMatchInlineSnapshot(
      `"{"version":3,"file":"main.js","names":[],"sources":["../main.js"],"sourcesContent":["export const test = 'res';\\n"],"mappings":";AAAA,MAAa,SAAO"}"`,
    );
  },
});
