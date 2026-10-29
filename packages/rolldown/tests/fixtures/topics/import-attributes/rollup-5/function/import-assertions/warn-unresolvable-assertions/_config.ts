import type { RolldownLog } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const logs: RolldownLog[] = [];

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    external: (id) => id.startsWith('external'),
    onLog(_level, log) {
      logs.push(log);
    },
  },
  afterTest() {
    expect(logs.map((log) => [log.code, log.loc?.line])).toEqual([
      ['INVALID_IMPORT_ATTRIBUTE', 1],
      ['INVALID_IMPORT_ATTRIBUTE', 2],
      ['INVALID_IMPORT_ATTRIBUTE', 3],
      ['INVALID_IMPORT_ATTRIBUTE', 4],
      ['INVALID_IMPORT_ATTRIBUTE', 5],
      ['INVALID_IMPORT_ATTRIBUTE', 6],
      ['INVALID_IMPORT_ATTRIBUTE', 7],
    ]);
  },
});
