import { defineTest } from 'rolldown-tests';
import { bundleAnalyzerPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';
import type { OutputAsset } from 'rolldown';

export default defineTest({
  config: {
    plugins: [bundleAnalyzerPlugin({ fileName: 'bundle-analysis.json' })],
  },
  async afterTest(output) {
    const asset = output.output.find(
      (o): o is OutputAsset => o.type === 'asset' && o.fileName === 'bundle-analysis.json',
    );
    expect(asset).toBeDefined();
    const data = JSON.parse(asset!.source as string);
    expect(data.meta.bundler).toBe('rolldown');
  },
});
