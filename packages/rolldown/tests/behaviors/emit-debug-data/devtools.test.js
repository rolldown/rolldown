import { rolldown } from 'rolldown';

import { existsSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';

// `.rolldown` dir is generated based on real cwd instead of `InputOptions.cwd`. We might be able to solve this in the future.
// For now, we just live with it.
const dotRolldownFileName = join(process.cwd(), '.rolldown');

test(`emit data for devtool`, async () => {
  // Clean up previous test data if exists
  if (existsSync(dotRolldownFileName)) {
    rmSync(dotRolldownFileName, { recursive: true, force: true });
  }

  await runBundle();

  const dotRolldownDir = readdirSync(dotRolldownFileName);
  expect(dotRolldownDir.length).toBe(1);
  const debugDataDir = readdirSync(
    join(dotRolldownFileName, dotRolldownDir[0]),
  );
  // Expect `logs.json` and `meta.json` exist
  expect(debugDataDir).toContain('logs.json');
  expect(debugDataDir).toContain('meta.json');

  // Ensure there are no invalid uninjected variables in the logs
  const variables = [
    '${build_id}',
    '${session_id}',
    '${hook_resolve_id_trigger}',
  ];
  const logsContent = readFileSync(
    join(
      dotRolldownFileName,
      dotRolldownDir[0],
      'logs.json',
    ),
  );
  for (const variable of variables) {
    expect(logsContent.includes(variable)).toBe(false);
  }
  const metaContent = readFileSync(
    join(
      dotRolldownFileName,
      dotRolldownDir[0],
      'meta.json',
    ),
  );
  for (const variable of variables) {
    expect(metaContent.includes(variable)).toBe(false);
  }

  async function runBundle() {
    const bundle = await rolldown({
      devtools: {},
      cwd: import.meta.dirname,
      input: join(import.meta.dirname, 'index.ts'),
      resolve: {
        // This needs to be explicitly set for now because oxc resolver doesn't
        // assume default exports conditions. Rolldown will ship with a default that
        // aligns with Vite in the future.
        conditionNames: ['import'],
      },
      plugins: [
        {
          name: 'test',
          renderChunk(code) {
            return code + '\n// test';
          },
        },
        {
          name: 'test-resolve',
          async resolveId(source, importer) {
            // Test this.resolve functionality
            if (source === './hello' && importer) {
              console.log(
                `[test-resolve] Resolving "${source}" from "${importer}"`,
              );

              // Use this.resolve to resolve the module
              const resolved = await this.resolve('./hello.ts', importer, {
                skipSelf: true, // Skip this plugin to avoid infinite recursion
              });

              if (resolved) {
                console.log(
                  `[test-resolve] Successfully resolved to: ${resolved.id}`,
                );
                return resolved;
              } else {
                console.log(`[test-resolve] Failed to resolve "${source}"`);
              }
            }
            return null;
          },
        },
      ],
    });
    await bundle.generate();
  }
});
