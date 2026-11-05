import type { RolldownLog } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const logs: RolldownLog[] = [];

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    external: (id) => {
      if (id === 'unresolved') return null;
      return true;
    },
    onLog(_level, log) {
      logs.push(log);
    },
  },
  afterTest(output) {
    expect(logs.map((log) => log.code)).toEqual(['UNRESOLVED_IMPORT']);
    const code = getOutputChunk(output)[0].code;
    // Rolldown sorts the attribute keys.
    expect(code).toContain('import { a } from "a" with { extra: "extra", type: "a" };');
    expect(code).toContain('import * as b from "b" with { type: "b" };');
    expect(code).toContain('export { c } from "c" with { type: "c" };');
    expect(code).toContain('export * from "d" with { type: "d" };');
    expect(code).toContain('import "unresolved" with { type: "e" };');
  },
});
