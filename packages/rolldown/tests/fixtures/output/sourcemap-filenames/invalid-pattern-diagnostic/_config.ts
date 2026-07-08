import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// An absolute `sourcemapFileNames` pattern is invalid and must be rejected. The diagnostic
// has to name the public camelCase option (`sourcemapFileNames`); before the fix it leaked the
// internal snake_case identifier `sourcemap_filename`, which is not a real option name.
export default defineTest({
  config: {
    output: {
      sourcemap: true,
      sourcemapFileNames: '/absolute.map',
    },
  },
  afterTest: () => {
    expect.unreachable('an absolute sourcemapFileNames pattern should fail validation');
  },
  catchError: (error: unknown) => {
    expect(String(error)).toContain('for "sourcemapFileNames"');
    expect(String(error)).not.toContain('sourcemap_filename');
  },
});
