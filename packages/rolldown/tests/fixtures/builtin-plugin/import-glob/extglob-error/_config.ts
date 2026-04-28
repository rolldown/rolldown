import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [viteImportGlobPlugin()],
  },
  catchError(err: unknown) {
    const message = String(err);
    expect(message).toContain('extglob');
    expect(message).toContain('!(*.d.ts)');
  },
});
