import type { ResolveIdResult } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test-plugin',
        resolveId(source): ResolveIdResult {
          if (source === './package.json') {
            // Add the attribute Node.js requires to import JSON.
            return { id: source, external: true, attributes: { type: 'json' } };
          }
          if (source === 'ext') {
            // `{}` removes the attributes.
            return { id: source, external: true, attributes: {} };
          }
          if (source === 'kept') {
            // Without `attributes`, the import keeps the ones it was written with.
            return { id: source, external: true };
          }
        },
      },
    ],
  },
  afterTest(output) {
    const code = getOutputChunk(output)[0].code;
    expect(code).toContain('import pkg from "./package.json" with { type: "json" };');
    expect(code).toContain('import ext from "ext";');
    expect(code).toContain('import kept from "kept" with { type: "json" };');
    // `import()` of an external carries the final attributes as well.
    expect(code).toContain('import("./package.json", { with: { type: "json" } })');
    expect(code).toContain('import("ext")');
    expect(code).toContain('import("kept", { with: { type: "json" } })');
  },
});
