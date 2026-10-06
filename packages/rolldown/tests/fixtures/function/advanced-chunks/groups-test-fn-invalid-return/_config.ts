import { stripAnsi } from 'consola/utils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// napi reports only the type of the batch array, so the shim checks each result and names the
// module.
export default defineTest({
  config: {
    optimization: {
      inlineConst: false,
    },
    // @ts-expect-error - this is intentionally wrong to trigger the error
    output: {
      codeSplitting: {
        groups: [
          {
            name: 'ab',
            test: (id: string) => (id.endsWith('a.js') ? 'yes' : false),
          },
        ],
      },
    },
  },
  catchError(err: any) {
    const message = stripAnsi(err.toString());
    expect(message).toContain('`output.codeSplitting.groups[].test` returned string for module');
    expect(message).toContain('a.js');
    expect(message).toContain('but expected a boolean, null or undefined.');
  },
});
