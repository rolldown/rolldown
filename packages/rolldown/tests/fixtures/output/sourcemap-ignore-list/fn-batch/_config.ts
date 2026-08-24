import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// One call decides every source of a sourcemap. The call order still follows the source order,
// and the returned flags still line up with it by index.
const seen: string[] = [];

export default defineTest({
  sequential: true,
  config: {
    output: {
      dir: 'dist',
      sourcemap: true,
      sourcemapIgnoreList: (source) => {
        seen.push(source);
        return source.includes('vendor');
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

    const parsed = JSON.parse(map.source as string);
    const ignored = parsed.x_google_ignoreList as number[];
    const vendorIndex = (parsed.sources as string[]).findIndex((source) =>
      source.includes('vendor'),
    );

    expect(vendorIndex).toBeGreaterThanOrEqual(0);
    expect(ignored).toStrictEqual([vendorIndex]);
    expect(seen).toStrictEqual(parsed.sources);
  },
});
