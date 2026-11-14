import { defineTest } from 'rolldown-tests';
import { esmExternalRequirePlugin } from 'rolldown/plugins';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [esmExternalRequirePlugin({ external: ['callable-producer', 'null-producer'] })],
    output: {
      format: 'esm',
      paths: {
        'callable-producer': '../callable-producer.js',
        'null-producer': '../null-producer.js',
      },
    },
  },
  async afterTest() {
    const producer = await import('./callable-producer.js' as string);
    const consumer = await import('./dist/main.js' as string);

    expect(Object.hasOwn(producer, 'module.exports')).toBe(true);
    expect(consumer.callable).toBe(producer['module.exports']);
    expect(consumer.callable(41)).toBe(42);
    expect(consumer.callable.kind).toBe('commonjs');
    expect(consumer.nullValue).toBe(null);
  },
});
