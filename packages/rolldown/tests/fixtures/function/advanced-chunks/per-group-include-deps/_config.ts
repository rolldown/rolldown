import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      codeSplitting: {
        groups: [
          {
            name: 'route-alpha',
            test: /routes[\\/]alpha/,
            includeDependenciesRecursively: false,
          },
          {
            name: 'shared',
            test: /shared[\\/]/,
          },
        ],
      },
    },
  },
  afterTest(output) {
    const routeAlpha = output.output.find(
      (chunk) => chunk.type === 'chunk' && chunk.fileName.startsWith('route-alpha-'),
    );
    if (routeAlpha?.type !== 'chunk') {
      throw new Error('route-alpha chunk not found');
    }
    expect(routeAlpha.moduleIds).not.toEqual(
      expect.arrayContaining([expect.stringMatching(/shared[\\/]util\.js$/)]),
    );
    expect(routeAlpha.moduleIds).toHaveLength(1);
    expect(routeAlpha.moduleIds).toMatchObject([
      expect.stringMatching(/routes[\\/]alpha[\\/]index\.js$/),
    ]);
  },
});
