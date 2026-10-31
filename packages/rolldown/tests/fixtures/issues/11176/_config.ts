import type { SourceMap } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

const isAction = (id: string) => /(replaced|omitted)\.js$/.test(id);
const combinedMaps: SourceMap[] = [];

// https://github.com/rolldown/rolldown/issues/11176: a transform that replaces a module and
// returns `map: { mappings: '' }`, or omits its map, has no mapping back to the original code,
// so the output map must not list the original. Rollup 4.64 lists only `main.js` here.
export default defineTest({
  config: {
    plugins: [
      {
        // Stands in for an earlier transform with a real map (e.g. TypeScript stripping).
        // The code is unchanged; the second segment names `readServerSecret` on line 2.
        name: 'earlier',
        transform(code, id) {
          if (!isAction(id) || !id.includes('earlier-then-')) return;
          return {
            code,
            map: {
              sources: [id],
              sourcesContent: [code],
              names: ['readServerSecret'],
              mappings: 'AAAA;SACSA',
            },
          };
        },
      },
      {
        // Replaces each action with a proxy that has no relation to the original code.
        name: 'replace',
        transform(_code, id) {
          if (!isAction(id)) return;
          const code = `export const save = () => 'proxy';\n`;
          return id.endsWith('omitted.js') ? { code } : { code, map: { mappings: '' } };
        },
      },
      {
        name: 'combined',
        transform(_code, id) {
          if (isAction(id)) combinedMaps.push(this.getCombinedSourcemap());
        },
      },
    ],
    onLog(level, log, handler) {
      // `earlier-then-omitted.js` omits its map on purpose.
      if (log.code !== 'SOURCEMAP_BROKEN') handler(level, log);
    },
    output: { sourcemap: true },
  },
  afterTest(output) {
    const map = getOutputChunk(output)[0].map!;
    expect({
      sources: map.sources,
      leakedContent: map.sourcesContent!.some((content) => content?.includes('readServerSecret')),
      leakedName: map.names.includes('readServerSecret'),
      combinedSources: combinedMaps.flatMap((combined) => combined.sources),
      combinedNames: combinedMaps.flatMap((combined) => combined.names),
    }).toStrictEqual({
      sources: ['../main.js'],
      leakedContent: false,
      leakedName: false,
      combinedSources: [],
      combinedNames: [],
    });
  },
});
