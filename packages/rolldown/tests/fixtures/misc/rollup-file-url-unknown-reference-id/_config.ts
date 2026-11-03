import { join } from 'node:path';
import { stripAnsi } from 'consola/utils';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {},
  catchError(e: any) {
    expect(e.errors).toHaveLength(1);
    const error = e.errors[0];
    expect(e.message).toContain('Unable to get file name for unknown file "doesntexistId"');
    expect(error).toMatchObject({
      code: 'FILE_NOT_FOUND',
      kind: 'FILE_NOT_FOUND',
      // The module holding the `import.meta.ROLLUP_FILE_URL_*` access, and where in it.
      id: join(import.meta.dirname, './main.js'),
      loc: { line: 1, column: 15, file: join(import.meta.dirname, './main.js') },
    });

    // The offending expression is also reported through a labelled code frame.
    const message = stripAnsi(error.message);
    expect(message).toContain(
      '[FILE_NOT_FOUND] Plugin error - Unable to get file name for unknown file "doesntexistId".',
    );
    expect(message).toContain('main.js:1:16');
    expect(message).toContain('import.meta.ROLLUP_FILE_URL_doesntexistId');
    expect(message).toContain('no emitted file has this reference id');
  },
});
