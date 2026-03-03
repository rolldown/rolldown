import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      codeSplitting: {
        groups: [
          {
            name: (file) => {
              const relative = file.replace(/^\/src\//, '');
              return relative.replace(/\..+$/, '');
            },
          },
        ],
      },
    },
  },
  catchError(error: any) {
    expect(error.message).toContain('Invalid substitution ');
    expect(error.message).toContain('can be neither absolute nor relative path');
  },
});
