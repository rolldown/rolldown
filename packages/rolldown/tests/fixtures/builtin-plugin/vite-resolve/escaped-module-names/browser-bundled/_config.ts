import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { browserModule, createResolver } from '../utils';

export default defineTest({
  config: {
    plugins: [createResolver(import.meta.dirname, true)],
  },
  async afterTest(output) {
    const { read } = await import(
      `data:text/javascript,${encodeURIComponent(output.output[0].code)}`
    );
    expect(read).toThrow(
      new Error(
        `Module "${browserModule}" has been externalized for browser compatibility. Cannot access "${browserModule}.test" in client code.  See https://vite.dev/guide/troubleshooting.html#module-externalized-for-browser-compatibility for more details.`,
      ),
    );
  },
});
