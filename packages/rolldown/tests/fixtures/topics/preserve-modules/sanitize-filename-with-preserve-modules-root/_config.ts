import type { OutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: {
      index: './src/index.js',
    },
    output: {
      preserveModules: true,
      preserveModulesRoot: 'src',
    },
  },
  afterTest: (output) => {
    const chunks = output.output.filter((item): item is OutputChunk => item.type === 'chunk');
    const moduleChunk = chunks.find((chunk) => chunk.facadeModuleId?.endsWith('+module.js'));
    expect(moduleChunk?.fileName).toBe('_module.js');
    expect(moduleChunk?.name).toBe('_module');
  },
});
