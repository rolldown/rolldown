import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { dev } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const TEST_TIMEOUT = 60_000;

test(
  'module queries on the engine handle',
  { timeout: TEST_TIMEOUT },
  async ({ onTestFinished }) => {
    const uniqueId = crypto.randomUUID().slice(0, 8);
    const dir = path.join(import.meta.dirname, 'temp', `dev-module-info-${uniqueId}`);
    fs.mkdirSync(dir, { recursive: true });
    const input = path.join(dir, 'main.js');
    const dep = path.join(dir, 'dep.js');
    fs.writeFileSync(input, 'import { value } from "./dep.js";\nconsole.log(value);\n');
    fs.writeFileSync(dep, 'export const value = 1;\n');

    const engine = await dev(
      { input, experimental: { devMode: true }, treeshake: false },
      { dir: path.join(dir, 'dist') },
      { watch: getDevWatchOptionsForCi() },
    );
    onTestFinished(async () => {
      await engine.close();
      if (!process.env.CI) {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    });

    engine.run().catch(() => {});
    await engine.ensureCurrentBuildFinish();

    const { moduleGraph } = engine;
    const ids = moduleGraph.getModuleIds();
    expect(ids).toContain(input);
    expect(ids).toContain(dep);

    const info = moduleGraph.getModuleInfo(input);
    expect(info).not.toBeNull();
    expect(info!.importedIds).toContain(dep);
    expect(moduleGraph.getModuleInfo(path.join(dir, 'missing.js'))).toBeNull();

    // The graph stays current across a full rebuild — full builds clear the
    // shared map in place instead of replacing it.
    engine.triggerFullBuild();
    await engine.ensureLatestBuildOutput();
    expect(moduleGraph.getModuleIds()).toContain(input);
    expect(moduleGraph.getModuleInfo(dep)).not.toBeNull();
  },
);
