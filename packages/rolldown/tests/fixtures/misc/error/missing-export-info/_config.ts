import { join } from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {},
  catchError(e: any) {
    const mainId = join(import.meta.dirname, 'main.js');

    expect(e.errors).toBeDefined();
    expect(e.errors.length).toBeGreaterThan(0);

    const error = e.errors[0];
    expect(error.id).toBe(mainId);
    expect(error.kind).toBe('MISSING_EXPORT');
    expect(error.code).toBe('MISSING_EXPORT');
    expect(error.loc).toBeDefined();
    expect(typeof error.loc.line).toBe('number');
    expect(typeof error.loc.column).toBe('number');
    expect(typeof error.pos).toBe('number');

    expect(error.message).toContain('missing');
    expect(error.message).toContain('dep.js');
  },
});
