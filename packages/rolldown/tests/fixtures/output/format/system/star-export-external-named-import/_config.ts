import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    external: ['external-lib'],
    output: {
      format: 'system',
    },
  },
  afterTest: (output) => {
    const chunk = output.output[0];
    if (chunk.type === 'chunk') {
      // Named import binding should have a setter assignment
      expect(chunk.code).toContain('x = module.x');
      // Star re-export should emit the _starExcludes object
      expect(chunk.code).toContain('_starExcludes');
      // Star re-export should emit the star loop setter
      expect(chunk.code).toContain('for (var name in module)');
      // Star re-export should emit exports(setter)
      expect(chunk.code).toContain('exports(setter)');
    }
  },
});
