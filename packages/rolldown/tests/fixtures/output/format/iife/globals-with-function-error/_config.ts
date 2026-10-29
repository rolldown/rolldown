import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      format: 'iife',
      name: 'module',
      globals: (name: string): string => {
        throw new Error(`unexpected global: ${name}`);
      },
    },
  },
  afterTest: () => {
    expect.unreachable('build should fail when the globals function throws');
  },
  catchError: (err: unknown) => {
    expect(String(err)).toContain('unexpected global: node:path');
  },
});
