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
    expect(logs).toEqual([]);
  },
});
