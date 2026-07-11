import { realpath } from 'node:fs/promises';
import { rolldown } from 'rolldown';
import { createRegistryGraphResolver, isBareDependency } from './resolver.mjs';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected an error semantics case');
const { variant, fixtureDirectory, entryPath } = options;
if (variant !== 'ordinary' && !/^worker-\d+$/.test(variant)) {
  throw new Error(`invalid variant: ${variant}`);
}
const canonicalFixture = await realpath(fixtureDirectory);
const normalizeText = (value) =>
  value
    .replaceAll(canonicalFixture, '<graph-error-fixture>')
    .replaceAll(fixtureDirectory, '<graph-error-fixture>');
const normalizeLog = (level, log) => ({
  level,
  code: log.code,
  pluginCode: log.pluginCode,
  message: normalizeText(log.message),
  id: log.id ? normalizeText(log.id) : undefined,
  plugin: log.plugin,
  hook: log.hook,
  loc: log.loc
    ? { ...log.loc, file: log.loc.file ? normalizeText(log.loc.file) : undefined }
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
    ? { ...error.loc, file: error.loc.file ? normalizeText(error.loc.file) : undefined }
    : undefined,
  frame: error.frame,
  stack: error.stack ? normalizeText(error.stack).split('\n').slice(0, 12).join('\n') : undefined,
});
const telemetry = {
  aliasRequests: 0,
  aliasResolutions: 0,
  nodeNextResolutions: 0,
  relativeNodeNextRequests: 0,
  relativeNodeNextResolutions: 0,
};
const resolver = createRegistryGraphResolver({ corpusDirectory: fixtureDirectory, telemetry });
const pluginOptions = { corpusDirectory: fixtureDirectory };
const transformPlugin =
  variant === 'ordinary'
    ? (await import('../../svelte-registry-graph-plugin/kernel.js')).svelteRegistryGraphPlugin(
        pluginOptions,
      )
    : (await import('../../svelte-registry-graph-plugin/index.js')).default(pluginOptions);
const logs = [];
let build;
try {
  build = await rolldown({
    cwd: fixtureDirectory,
    input: entryPath,
    external: (source) => isBareDependency(source),
    logLevel: 'debug',
    onLog(level, log) {
      logs.push(normalizeLog(level, log));
    },
    plugins: [resolver, transformPlugin],
  });
  await build.generate({ format: 'esm', sourcemap: true });
  await build.close();
  build = undefined;
  console.log(JSON.stringify({ variant, success: true, logs }));
} catch (error) {
  console.log(
    JSON.stringify({ variant, success: false, logs, error: normalizeError(error), telemetry }),
  );
} finally {
  await build?.close();
}
