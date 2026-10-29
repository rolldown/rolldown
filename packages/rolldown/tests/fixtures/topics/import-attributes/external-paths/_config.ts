import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const seen: string[] = [];

export default defineTest({
  config: {
    external: ['ext'],
    experimental: { importAttributes: true },
    output: {
      // Called with the raw id, like in Rollup.
      paths: (id) => {
        seen.push(id);
        return id === 'ext' ? 'https://cdn.example/ext.json' : id;
      },
    },
  },
  afterTest(output) {
    const code = getOutputChunk(output)[0].code;
    expect(code).toContain(
      'import data from "https://cdn.example/ext.json" with { type: "json" };',
    );
    // `import()` with options gets the same path as a static import.
    expect(code).toContain('import("https://cdn.example/ext.json", { with: { type: "json" } })');
    expect(seen).toStrictEqual(['ext']);
  },
});
