import { isSingleThread } from '@tests/runtime-flavor';
import path from 'node:path';
import { rolldown } from 'rolldown';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

test.skipIf(isSingleThread)(
  'class-instance builtin callbacks reject bundle.close() without deadlocking',
  { timeout: 5_000 },
  async ({ onTestFinished }) => {
    const fixtureDir = path.join(
      import.meta.dirname,
      'fixtures/builtin-plugin/dynamic-import-vars/basic',
    );
    let bundle!: Awaited<ReturnType<typeof rolldown>>;

    class DynamicImportVarsConfig {
      resolverCalls = 0;
      resolverRejectedClose = false;

      async resolver(id: string): Promise<string> {
        this.resolverCalls += 1;
        await expect(bundle.close()).rejects.toThrow(/active JavaScript callbacks/);
        this.resolverRejectedClose = true;
        return id.replace('@', path.join(fixtureDir, 'dir/a'));
      }
    }

    const config = new DynamicImportVarsConfig();
    bundle = await rolldown({
      input: path.join(fixtureDir, 'main.js'),
      plugins: [viteDynamicImportVarsPlugin(config), viteImportGlobPlugin()],
    });
    onTestFinished(() => bundle.close().catch(() => {}));

    await bundle.generate();
    await bundle.close();

    expect(config.resolverCalls).toBeGreaterThan(0);
    expect(config.resolverRejectedClose).toBe(true);
  },
);

test(
  'get-trap builtin callbacks survive close-callback option wrapping',
  { timeout: 10_000 },
  async ({ onTestFinished }) => {
    const fixtureDir = path.join(
      import.meta.dirname,
      'fixtures/builtin-plugin/dynamic-import-vars/basic',
    );
    const resolvedIds: string[] = [];
    const resolverReceivers: unknown[] = [];
    // The callback lives only in the `get` trap. Wrapping the builtin options
    // by cloning their own descriptors cannot see a virtual property, so the
    // clone would reach native without a `resolver` and never call back.
    const target: { exclude?: unknown; include?: unknown } = {};
    const resolver = function (this: unknown, id: string): string {
      resolvedIds.push(id);
      resolverReceivers.push(this);
      return id.replace('@', path.join(fixtureDir, 'dir/a'));
    };
    const config = new Proxy(target, {
      get(proxyTarget, key, receiver) {
        if (key === 'resolver') return resolver;
        return Reflect.get(proxyTarget, key, receiver);
      },
    }) as Parameters<typeof viteDynamicImportVarsPlugin>[0];
    expect(Object.getOwnPropertyDescriptor(target, 'resolver')).toBeUndefined();

    const bundle = await rolldown({
      input: path.join(fixtureDir, 'main.js'),
      plugins: [viteDynamicImportVarsPlugin(config), viteImportGlobPlugin()],
    });
    onTestFinished(() => bundle.close().catch(() => {}));

    await bundle.generate();
    await bundle.close();

    // `main.js` imports a bare specifier (`@/${name}.js`), so native genuinely
    // consults the resolver instead of resolving the glob on its own.
    expect(resolvedIds).toEqual(['@/*.js']);
    expect(resolverReceivers).toEqual([config]);
  },
);
