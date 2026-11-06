import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {},
  catchError(e: any) {
    const id = join(import.meta.dirname, 'data.json');
    const error = e.errors[0];
    expect(error.code).toBe('JSON_PARSE');
    expect(error.id).toBe(id);
    expect(error.loc).toMatchObject({ line: 3, column: 0, file: id });
  },
});
