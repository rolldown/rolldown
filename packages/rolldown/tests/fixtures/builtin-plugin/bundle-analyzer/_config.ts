import { defineTest } from 'rolldown-tests';
import { bundleAnalyzerPlugin } from 'rolldown/experimental';
import type { OutputAsset } from 'rolldown';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [bundleAnalyzerPlugin()],
  },
  async afterTest(output) {
    const asset = output.output.find(
      (o): o is OutputAsset => o.type === 'asset' && o.fileName === 'analyze-data.json',
    );
    expect(asset).toBeDefined();
    const data = JSON.parse(asset!.source as string);
    expect(data.meta.bundler).toBe('rolldown');
    expect(data.meta.timestamp).toBeTypeOf('number');
    expect(data.chunks.length).toBeGreaterThan(0);
    expect(data.modules.length).toBeGreaterThan(0);
  },
});
