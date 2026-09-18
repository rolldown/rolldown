import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// One call rewrites every source of a sourcemap. The call order still follows the source order,
// and the returned paths still line up with it by index.
const seen: string[] = [];

export default defineTest({
  sequential: true,
  config: {
    output: {
      dir: 'dist',
      sourcemap: true,
      sourcemapPathTransform: (source) => {
        seen.push(source);
        return `prefixed/${source}`;
      },
    },
  },
  afterTest: (output) => {
    const map = output.output.find(
      (asset) => asset.type === 'asset' && asset.fileName.endsWith('.map'),
    );

    if (map?.type !== 'asset') {
      throw new Error('should emit a sourcemap');
    }

    const sources = JSON.parse(map.source as string).sources as string[];

    expect(seen.length).toBeGreaterThan(1);
    expect(sources).toStrictEqual(seen.map((source) => `prefixed/${source}`));
  },
});
