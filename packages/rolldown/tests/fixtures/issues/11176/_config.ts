import type { SourceMap } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const SECRET = 'SERVER_ONLY_SECRET';
const isAction = (id: string) => /action(-without-earlier-map)?\.js$/.test(id);
const combinedSourcemaps = new Map<string, SourceMap>();

function expectNoLeak(map: { sources: (string | null)[]; sourcesContent?: (string | null)[] }) {
  expect(map.sources.filter((source) => source && isAction(source))).toStrictEqual([]);
  expect((map.sourcesContent ?? []).filter((content) => content?.includes(SECRET))).toStrictEqual(
    [],
  );
}

// https://github.com/rolldown/rolldown/issues/11176: a transform returning `map: { mappings: '' }`
// has no mapping back to the module's previous code, so that code must stay out of the emitted
// sourcemap, like in Rollup. `action.js` is also transformed by an earlier plugin with a real map
// (e.g. TypeScript stripping), which must not bring the original back.
export default defineTest({
  config: {
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'earlier',
        transform(code, id) {
          if (!id.endsWith('/action.js')) return;
          return { code: `/* earlier */\n${code}`, map: { mappings: ';AAAA' } };
        },
      },
      {
        name: 'replace',
        transform(_code, id) {
          if (!isAction(id)) return;
          const name = id.endsWith('/action.js') ? 'save' : 'remove';
          return { code: `export const ${name} = () => 'proxy';\n`, map: { mappings: '' } };
        },
      },
      {
        name: 'combined-sourcemap',
        transform(_code, id) {
          if (!isAction(id)) return;
          combinedSourcemaps.set(id, this.getCombinedSourcemap());
        },
      },
    ],
  },
  afterTest(output) {
    const chunk = getOutputChunk(output)[0];
    expect(chunk.code).toContain('proxy');
    expect(chunk.code).not.toContain(SECRET);
    expectNoLeak(chunk.map!);
    expect(chunk.map!.sources.some((source) => source?.endsWith('main.js'))).toBe(true);

    expect(combinedSourcemaps.size).toBe(2);
    for (const map of combinedSourcemaps.values()) {
      expectNoLeak(map);
    }
  },
});
