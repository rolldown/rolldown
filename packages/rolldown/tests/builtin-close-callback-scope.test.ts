import { isSingleThread } from '@tests/runtime-flavor';
import path from 'node:path';
import { rolldown } from 'rolldown';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

test.skipIf(isSingleThread)(
  'class-instance builtin callbacks can await bundle.close()',
  { timeout: 5_000 },
  async ({ onTestFinished }) => {
    const fixtureDir = path.join(
      import.meta.dirname,
      'fixtures/builtin-plugin/dynamic-import-vars/basic',
    );
    let bundle!: Awaited<ReturnType<typeof rolldown>>;

    class DynamicImportVarsConfig {
      resolverCalls = 0;
      resolverCompleted = false;

      async resolver(id: string): Promise<string> {
        this.resolverCalls += 1;
        await bundle.close();
        this.resolverCompleted = true;
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
    expect(config.resolverCompleted).toBe(true);
  },
);
