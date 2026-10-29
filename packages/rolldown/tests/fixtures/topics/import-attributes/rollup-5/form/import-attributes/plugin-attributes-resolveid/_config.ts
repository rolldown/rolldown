import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: path.join(import.meta.dirname, 'main.js'),
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test',
        resolveId(source, _importer, { attributes, isEntry }) {
          return {
            id: source,
            external: !isEntry,
            attributes: Object.fromEntries(Object.keys(attributes).map((key) => [key, 'changed'])),
          };
        },
      },
    ],
  },
  afterTest(output) {
    const code = getOutputChunk(output)[0].code;
    // Rolldown sorts the attribute keys.
    expect(code).toContain('import { a } from "a" with { extra: "changed", type: "changed" };');
    expect(code).toContain('import * as b from "b" with { type: "changed" };');
    expect(code).toContain('export { c } from "c" with { type: "changed" };');
    expect(code).toContain('export * from "d" with { type: "changed" };');
    expect(code).toContain('import "e";');
    expect(code).toContain('import("f", { with: { type: "changed" } });');
    expect(code).toContain('import("g");');
  },
});
