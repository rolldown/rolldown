import { rolldown } from 'rolldown';
import { esmExternalRequirePlugin } from 'rolldown/plugins';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [esmExternalRequirePlugin({ external: ['callable-producer', 'null-producer'] })],
    output: {
      format: 'esm',
      entryFileNames: 'consumer.mjs',
      paths: {
        'callable-producer': './callable-producer.mjs',
        'null-producer': './null-producer.mjs',
      },
    },
  },
  async afterTest() {
    const producerBuild = await rolldown({
      cwd: import.meta.dirname,
      input: {
        'callable-producer': 'callable-producer.js',
        'null-producer': 'null-producer.js',
      },
    });
    await producerBuild.write({
      dir: 'dist',
      format: 'esm',
      entryFileNames: '[name].mjs',
    });
    await producerBuild.close();

    const producer = await import('./dist/callable-producer.mjs' as string);
    const consumer = await import('./dist/consumer.mjs' as string);

    expect(Object.hasOwn(producer, 'module.exports')).toBe(true);
    expect(typeof consumer.callable).toBe('function');
    expect(consumer.callable).toBe(producer['module.exports']);
    expect(consumer.callable(41)).toBe(42);
    expect(consumer.callable.kind).toBe('commonjs');
    expect(consumer.nullValue).toBe(null);
  },
});
