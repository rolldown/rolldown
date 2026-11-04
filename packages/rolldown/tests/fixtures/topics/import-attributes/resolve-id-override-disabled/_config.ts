import type { ResolveIdResult } from 'rolldown';
import { stringifyModuleId } from 'rolldown/utils';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// Without `experimental.importAttributes`, the attributes a `resolveId` hook returns are ignored,
// and an external keeps the ones it was written with.
export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source): ResolveIdResult {
          if (source === './package.json') {
            return { id: source, external: true, attributes: { type: 'json' } };
          }
          if (source === 'ext') {
            return { id: source, external: true, attributes: {} };
          }
          if (source === 'kept') {
            return { id: source, external: true, meta: { kept: true } };
          }
        },
        buildEnd() {
          const id = [...this.getModuleIds()].find((id) => id.startsWith('kept?'));
          expect(id).toBe(stringifyModuleId('kept', { type: 'json' }));
          // Hooks saw no attributes, so the options are found by the raw id.
          expect(this.getModuleInfo(id!)!.meta).toEqual({ kept: true });
        },
      },
    ],
  },
  afterTest(output) {
    const code = getOutputChunk(output)[0].code;
    expect(code).toContain('import pkg from "./package.json";');
    expect(code).toContain('import ext from "ext" with { type: "json" };');
    expect(code).toContain('import kept from "kept" with { type: "json" };');
    expect(code).toContain('import("./package.json")');
    expect(code).toContain('import("ext", { with: { type: "json" } })');
    expect(code).toContain('import("kept", { with: { type: "json" } })');
  },
});
