import { stripAnsi } from 'consola/utils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// napi reports only the type of the batch array, so the shim checks each result and names the
// module.
export default defineTest({
  config: {
    // @ts-expect-error - this is intentionally wrong to trigger the error
    output: {
      codeSplitting: {
        groups: [
          {
            name: (id: string) => id.length,
          },
        ],
      },
    },
  },
  catchError(err: any) {
    const message = stripAnsi(err.toString());
    expect(message).toContain('`output.codeSplitting.groups[].name` returned number for module');
    expect(message).toContain('main.js');
    expect(message).toContain('but expected a string, null or undefined.');
  },
});
