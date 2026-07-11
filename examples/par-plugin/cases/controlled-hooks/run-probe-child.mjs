import { createHash } from 'node:crypto';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import nodePath from 'node:path';
import { rolldown } from 'rolldown';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected probe options');
const { variant, mode } = options;
const corpusDirectory = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-parallel-hook-probe-'));
let build;

try {
  let entrySource;
  if (mode === 'state') {
    entrySource = `${Array.from(
      { length: 32 },
      (_, index) => `import 'controlled-state:${index}';`,
    ).join('\n')}\n`;
  } else if (mode === 'reentrant') {
    entrySource = "import 'controlled-reentrant:outer';\n";
  } else if (mode === 'resolve-error') {
    entrySource = "import 'controlled-resolve-error:value';\n";
  } else if (mode === 'load-error') {
    entrySource = "import 'controlled-load-error:value';\n";
  } else if (mode === 'filter-miss') {
    entrySource = 'globalThis.__filterMissProbe = true;\n';
  } else {
    throw new Error(`invalid correctness probe mode: ${mode}`);
  }
  await writeFile(nodePath.join(corpusDirectory, 'entry.js'), entrySource);

  const measuredPlugin =
    variant === 'ordinary'
      ? (await import('../../controlled-hooks-plugin/probe-impl.js')).createProbePlugin({ mode })
      : (await import('../../controlled-hooks-plugin/probe-index.js')).default({ mode });
  const supportPlugin = {
    name: 'controlled-correctness-probe-support',
    resolveId(specifier) {
      if (specifier.startsWith('controlled-load-error:')) return `\0${specifier}`;
      return null;
    },
    load(id) {
      if (id.startsWith('\0controlled-state:')) {
        const [, thread, call, index] = id.split(':');
        return `globalThis.__stateProbe = (globalThis.__stateProbe || []).concat([[${thread}, ${call}, ${index}]]);\n`;
      }
      if (id === '\0controlled-reentrant-result') {
        return 'globalThis.__reentrantProbe = true;\n';
      }
      return null;
    },
  };

  build = await rolldown({
    cwd: corpusDirectory,
    input: 'entry.js',
    logLevel: 'silent',
    treeshake: false,
    plugins: [measuredPlugin, supportPlugin],
  });
  const result = await build.generate({ format: 'esm' });
  await build.close();
  build = undefined;
  const code = result.output
    .filter((output) => output.type === 'chunk')
    .map((output) => output.code)
    .join('\n');
  const stateTuples = [...code.matchAll(/\.concat\(\[\[\s*(\d+),\s*(\d+),\s*(\d+)\s*\]\]\)/g)].map(
    (match) => match.slice(1).map(Number),
  );
  console.log(
    JSON.stringify({
      variant,
      mode,
      outputBytes: Buffer.byteLength(code),
      outputHash: createHash('sha256').update(code).digest('hex'),
      stateTuples,
      ...(process.env.CONTROLLED_HOOK_PROBE_PRINT_CODE === '1' ? { code } : {}),
    }),
  );
} finally {
  await build?.close();
  await rm(corpusDirectory, { recursive: true, force: true });
}
