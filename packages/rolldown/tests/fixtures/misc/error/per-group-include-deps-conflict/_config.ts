import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { stripVTControlCharacters } from 'node:util';

export default defineTest({
  config: {
    input: 'main.js',
    output: {
      preserveEntrySignatures: 'strict',
      codeSplitting: {
        groups: [
          {
            name: 'route',
            test: /routes/,
            includeDependenciesRecursively: false,
          },
        ],
      },
    },
  },
  catchError(e: any) {
    const message = stripVTControlCharacters(e.message);
    expect(message).toContain('includeDependenciesRecursively');
    expect(message).toContain('preserveEntrySignatures');
  },
});
