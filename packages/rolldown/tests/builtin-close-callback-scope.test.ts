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
