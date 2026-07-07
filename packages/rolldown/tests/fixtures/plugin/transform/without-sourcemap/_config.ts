import { defineTest } from 'rolldown-tests';
import { getLocation, getOutputAsset, getOutputChunk } from 'rolldown-tests/utils';
import { SourceMapConsumer } from 'source-map';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform(code) {
          return {
            code: code + '\nconsole.log("added")',
            map: null,
          };
        },
      },
    ],
    output: {
      sourcemap: true,
    },
  },
  afterTest: async (output) => {
    const code = getOutputChunk(output)[0].code;
    const map = getOutputAsset(output)[0].source as string;
    const smc = await new SourceMapConsumer(JSON.parse(map));

    // When a transform hook returns `map: null` together with changed code,
    // the module's original (pre-transform) code is preserved as the source
    // content — the injected code must not leak into `sourcesContent` (#3092).
    // The original code stays mapped, while the injected code is left unmapped.
    const generatedLoc = getLocation(code, code.indexOf(`"main"`));
    const originalLoc = smc.originalPositionFor(generatedLoc);
    expect(originalLoc.line).toBe(1);
    expect(originalLoc.column).toBe(12);
    expect(smc.sourceContentFor(originalLoc.source!)).toBe("console.log('main');\n");

    const generatedLoc2 = getLocation(code, code.indexOf(`"added"`));
    const originalLoc2 = smc.originalPositionFor(generatedLoc2);
    expect(originalLoc2.line).toBe(null);
    expect(originalLoc2.column).toBe(null);
    expect(originalLoc2.source).toBe(null);
  },
});
