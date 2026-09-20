import { build } from 'rolldown';
import { expect, test } from 'vitest';

// A plugin-bound build (>= 3 s of plugin time) makes the terminal close report
// PLUGIN_TIMINGS through `onLog`, so a throwing `onLog` rejects the close. The
// bundle's cleanup is retryable, the retry succeeds, and `build()` used to
// resolve with the output anyway — dropping the user's error, which main's
// `finally { await build.close() }` rejects with. The workerd sibling
// (`workerd-build.test.ts`, 'a close() rejected after the build still releases
// the caller-owned instance') pins the same contract on the dist.
//
// `BuildTimings::is_plugin_bound` (crates/rolldown_plugin/src/types/build_timings.rs)
// gates that report on two clocks: the build must run >= 3 s, AND its link stage
// must be non-zero and under a hundredth of the rest of the build. The sleep
// covers the first; the module graph keeps the link stage clear of the
// quantization floor on the WebAssembly clock.
const LINK_GRAPH_MODULES = 200;

function makeVirtualGraph(moduleCount: number): Map<string, string> {
  const files = new Map<string, string>();
  files.set('virt:util.js', 'export function greet(name) { return `hello ${name}`; }\n');
  for (let index = 0; index < moduleCount; index++) {
    const next =
      index + 1 < moduleCount
        ? `import { value as next } from 'virt:mod-${index + 1}.js';`
        : 'const next = 1;';
    files.set(
      `virt:mod-${index}.js`,
      [
        next,
        "import { greet } from 'virt:util.js';",
        `export const value = ${index} + next;`,
        `export const label_${index} = greet('mod-${index}');`,
      ].join('\n'),
    );
  }
  return files;
}

test(
  'build rejects with the onLog error its terminal close reported',
  { timeout: 180_000 },
  async () => {
    const files = makeVirtualGraph(LINK_GRAPH_MODULES);
    const codes: string[] = [];
    const onLogError = new Error('onLog threw on the plugin-timings warning');

    const rejection: unknown = await build({
      input: 'virt:entry.js',
      write: false,
      plugins: [
        {
          // Listed first so it owns the entry: the sleep must run exactly once.
          name: 'slow-load',
          resolveId: (id) => (id === 'virt:entry.js' ? id : undefined),
          load: async (id) => {
            if (id !== 'virt:entry.js') return undefined;
            await new Promise((resolve) => setTimeout(resolve, 4_000));
            return "import { value } from 'virt:mod-0.js';\nexport const a = value;\n";
          },
        },
        {
          name: 'virtual-graph',
          resolveId: (id) => (files.has(id) ? id : undefined),
          load: (id) => files.get(id),
        },
      ],
      onLog(_level, log) {
        codes.push(log.code ?? '');
        if (log.code === 'PLUGIN_TIMINGS') throw onLogError;
      },
      output: { format: 'esm' },
    }).catch((error: unknown) => error);

    expect(codes).toContain('PLUGIN_TIMINGS');
    expect(rejection).toBe(onLogError);
  },
);
