import { build, rolldown, watch } from 'rolldown';
import { scan } from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

describe('HMR validation', () => {
  test('should throw error when using HMR with build API', async () => {
    await expect(
      build({
        input: 'virtual',
        experimental: { hmr: true },
        plugins: [
          {
            name: 'test',
            resolveId(id) {
              if (id === 'virtual') return '\0' + id;
            },
            load(id) {
              if (id === '\0virtual') return 'export default 1';
            },
          },
        ],
      }),
    ).rejects.toThrow(/experimental\.hmr.*only supported with.*dev.*API/i);
  });

  test('should throw error when using HMR with rolldown API and generate', async () => {
    const bundle = await rolldown({
      input: 'virtual',
      experimental: { hmr: true },
      plugins: [
        {
          name: 'test',
          resolveId(id) {
            if (id === 'virtual') return '\0' + id;
          },
          load(id) {
            if (id === '\0virtual') return 'export default 1';
          },
        },
      ],
    });

    await expect(bundle.generate()).rejects.toThrow(
      /experimental\.hmr.*only supported with.*dev.*API/i,
    );

    await bundle.close();
  });

  test('should throw error when using HMR with rolldown API and write', async () => {
    const bundle = await rolldown({
      input: 'virtual',
      experimental: { hmr: true },
      plugins: [
        {
          name: 'test',
          resolveId(id) {
            if (id === 'virtual') return '\0' + id;
          },
          load(id) {
            if (id === '\0virtual') return 'export default 1';
          },
        },
      ],
    });

    await expect(bundle.write()).rejects.toThrow(
      /experimental\.hmr.*only supported with.*dev.*API/i,
    );

    await bundle.close();
  });

  test('should throw error when using HMR with scan API', async () => {
    await expect(
      scan({
        input: 'virtual',
        experimental: { hmr: true },
        plugins: [
          {
            name: 'test',
            resolveId(id) {
              if (id === 'virtual') return '\0' + id;
            },
            load(id) {
              if (id === '\0virtual') return 'export default 1';
            },
          },
        ],
      }),
    ).rejects.toThrow(/experimental\.hmr.*only supported with.*dev.*API/i);
  });

  // Note: The watch API currently doesn't properly surface construction errors synchronously
  // due to the async nature of createWatcher(). However, the validation is now properly
  // done in the Rust layer when BindingWatcher is constructed, which will cause the
  // promise to reject. This test verifies the error is thrown (as an unhandled rejection).
  test('should throw error when using HMR with watch API', async () => {
    // Track unhandled rejections
    const rejections: Error[] = [];
    const handler = (reason: Error) => {
      rejections.push(reason);
    };
    process.on('unhandledRejection', handler);

    try {
      watch({
        input: 'virtual',
        experimental: { hmr: true },
        plugins: [
          {
            name: 'test',
            resolveId(id) {
              if (id === 'virtual') return '\0' + id;
            },
            load(id) {
              if (id === '\0virtual') return 'export default 1';
            },
          },
        ],
      });

      // Wait for the unhandled rejection
      await new Promise((resolve) => setTimeout(resolve, 100));
      
      expect(rejections.length).toBeGreaterThan(0);
      expect(rejections[0].message).toMatch(/experimental\.hmr.*only supported with.*dev.*API/i);
    } finally {
      process.off('unhandledRejection', handler);
    }
  });

  // FIXME: watch API validation is tested manually because watch() does not handle errors properly
  //        see https://github.com/rolldown/rolldown/issues/6482#:~:text=Watch%20mode%20does%20not%20handle%20errors%20in%20options%20hook%20and%20causes%20promise%20rejections

  test('should validate HMR after options hook in build API', async () => {
    // This test verifies that validation happens after the options hook runs
    await expect(
      build({
        input: 'virtual',
        plugins: [
          {
            name: 'test-add-hmr',
            options(opts) {
              // Plugin adds HMR in options hook
              return {
                ...opts,
                experimental: { hmr: true },
              };
            },
            resolveId(id) {
              if (id === 'virtual') return '\0' + id;
            },
            load(id) {
              if (id === '\0virtual') return 'export default 1';
            },
          },
        ],
      }),
    ).rejects.toThrow(/experimental\.hmr.*only supported with.*dev.*API/i);
  });

  test('should validate HMR after options hook in rolldown API', async () => {
    // This test verifies that validation happens after the options hook runs
    const bundle = await rolldown({
      input: 'virtual',
      plugins: [
        {
          name: 'test-add-hmr',
          options(opts) {
            // Plugin adds HMR in options hook
            return {
              ...opts,
              experimental: { hmr: true },
            };
          },
          resolveId(id) {
            if (id === 'virtual') return '\0' + id;
          },
          load(id) {
            if (id === '\0virtual') return 'export default 1';
          },
        },
      ],
    });

    await expect(bundle.generate()).rejects.toThrow(
      /experimental\.hmr.*only supported with.*dev.*API/i,
    );

    await bundle.close();
  });
});
