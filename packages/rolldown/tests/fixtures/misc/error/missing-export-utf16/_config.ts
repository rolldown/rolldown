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

    // File structure:
    // Line 1: "// 💣 emoji takes 2 UTF-16 code units"
    // Line 2: "import { missing } from './dep.js';"

    // The error should point to line 2 (1-indexed)
    expect(error.loc.line).toBe(2);
    // The column should account for UTF-16 encoding (0-indexed)
    // "import { " = 9 UTF-16 code units
    // So the column where "missing" starts should be 9
    expect(error.loc.column).toBe(9);

    // Position calculation (UTF-16, 0-indexed from start of file):
    // Line 1: "// 💣 emoji takes 2 UTF-16 code units" + \n
    //   "//" (2) + " " (1) + "💣" (2 UTF-16 units!) + " emoji takes 2 UTF-16 code units" (32) + \n (1) = 38
    // Line 2 position at "missing": 38 + "import { " (9) = 47
    const expectedPos = 38 + 9; // 47
    expect(error.pos).toBe(expectedPos);
  },
});
