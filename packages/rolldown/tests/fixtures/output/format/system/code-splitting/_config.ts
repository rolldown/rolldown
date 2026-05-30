import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      format: 'system',
    },
  },
  afterTest: (output) => {
    // Dynamic import produces two chunks: entry + lazy chunk
    expect(output.output).toHaveLength(2);

    const entry = output.output.find((c) => c.type === 'chunk' && c.isEntry);
    const lazy = output.output.find((c) => c.type === 'chunk' && !c.isEntry);

    expect(entry).toBeDefined();
    expect(lazy).toBeDefined();

    // Entry uses module.import() instead of import() in SystemJS
    if (entry?.type === 'chunk') {
      expect(entry.code).toContain('module.import(');
    }

    // Lazy chunk is wrapped in System.register with an exports function
    if (lazy?.type === 'chunk') {
      expect(lazy.code).toContain('System.register(');
      expect(lazy.code).toContain('exports(');
    }
  },
});
