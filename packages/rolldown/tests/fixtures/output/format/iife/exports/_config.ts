import type { LogLevel, RollupLog } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const logs: Array<{ level: LogLevel; log: RollupLog }> = [];

export default defineTest({
  sequential: true,
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      format: 'iife',
    },
    onLog(level, log) {
      logs.push({ level, log });
    },
  },
  afterTest: (output) => {
    expect(logs).toHaveLength(2);
    expect(logs[0].level).toBe('warn');
    expect(logs[0].log.code).toBe('MISSING_NAME_OPTION_FOR_IIFE_EXPORT');
    expect(logs[1].level).toBe('warn');
    expect(logs[1].log.code).toBe('MISSING_GLOBAL_NAME');
    expect(logs[1].log.message).toContain(
      'No name was provided for external module "node:path" in "output.globals" – guessing "node_path".',
    );

    expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function(exports, node_path) {

      Object.defineProperties(exports, { __esModule: { value: true }, [Symbol.toStringTag]: { value: 'Module' } });

      //#region main.js
      	var main_default = node_path.join;

      //#endregion
      exports.default = main_default;
      return exports;
      })({}, node_path);"
    `);
  },
});
