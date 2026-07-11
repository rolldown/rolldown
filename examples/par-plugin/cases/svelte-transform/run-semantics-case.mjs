import { createHash } from 'node:crypto';
import { realpath } from 'node:fs/promises';
import { rolldown } from 'rolldown';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected a JSON semantics case');
const { variant, fixtureDirectory, entryPath } = options;
const workerMatch = /^worker-(\d+)$/.exec(variant);
if (variant !== 'ordinary' && !workerMatch) throw new Error(`invalid variant: ${variant}`);

const fixturePaths = [await realpath(fixtureDirectory), fixtureDirectory].sort(
  (left, right) => right.length - left.length,
);
const normalizeText = (value) => {
  let normalized = value;
  for (const path of fixturePaths) normalized = normalized.replaceAll(path, '<semantics-fixture>');
  return normalized;
};
const normalizeLog = (level, log) => ({
  level,
  code: log.code,
  pluginCode: log.pluginCode,
  message: normalizeText(log.message),
  id: log.id ? normalizeText(log.id) : undefined,
  plugin: log.plugin,
  hook: log.hook,
  loc: log.loc
    ? {
        ...log.loc,
        file: log.loc.file ? normalizeText(log.loc.file) : undefined,
      }
    : undefined,
  frame: log.frame,
});
const normalizeError = (error) => ({
  name: error.name,
  code: error.code,
  message: normalizeText(error.message),
  id: error.id ? normalizeText(error.id) : undefined,
  plugin: error.plugin,
  hook: error.hook,
  loc: error.loc
    ? {
        ...error.loc,
        file: error.loc.file ? normalizeText(error.loc.file) : undefined,
      }
    : undefined,
  frame: error.frame,
  stack: error.stack ? normalizeText(error.stack).split('\n').slice(0, 8).join('\n') : undefined,
});

const logs = [];
let build;
try {
  const pluginOptions = { corpusDirectory: fixtureDirectory };
  const plugin =
    variant === 'ordinary'
      ? (await import('../../svelte-transform-plugin/kernel.js')).svelteTransformPlugin(
          pluginOptions,
        )
      : (await import('../../svelte-transform-plugin/index.js')).default(pluginOptions);
  build = await rolldown({
    cwd: fixtureDirectory,
    input: entryPath,
    logLevel: 'debug',
    external: (_source, importer) => Boolean(importer && !importer.endsWith('/entry.js')),
    onLog(level, log) {
      logs.push(normalizeLog(level, log));
    },
    plugins: [plugin],
    treeshake: false,
  });
  const result = await build.generate({ format: 'esm', sourcemap: true });
  await build.close();
  build = undefined;
  const codeHash = createHash('sha256');
  const mapHash = createHash('sha256');
  for (const chunk of result.output.filter((output) => output.type === 'chunk')) {
    const map = typeof chunk.map === 'string' ? chunk.map : JSON.stringify(chunk.map);
    codeHash.update(normalizeText(chunk.code));
    mapHash.update(normalizeText(map));
  }
  console.log(
    JSON.stringify({
      variant,
      success: true,
      logs,
      outputCodeHash: codeHash.digest('hex'),
      outputMapHash: mapHash.digest('hex'),
    }),
  );
} catch (error) {
  console.log(JSON.stringify({ variant, success: false, logs, error: normalizeError(error) }));
} finally {
  await build?.close();
}
