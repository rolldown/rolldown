import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'my-plugin',
        transform(_code, id) {
          if (id.startsWith('\0')) return;
          return 'export const x = ;\n';
        },
      },
    ],
  },
  catchError(e: any) {
    const id = join(import.meta.dirname, 'main.js');
    const error = e.errors[0];
    expect(error.code).toBe('PARSE_ERROR');
    expect(error.id).toBe(id);
    expect(error.loc).toMatchObject({ line: 1, column: 17, file: id });
    expect(error.frame).toBe(`1: export const x = ;\n${' '.repeat(20)}^`);
  },
});
