import nodePath from 'node:path';
import { rolldown } from 'rolldown';

const variant = process.argv[2];
if (!['full-ordinary', 'ordinary', 'worker-1'].includes(variant)) {
  throw new Error(`invalid failure variant: ${variant}`);
}
const root = process.argv[3];
if (!nodePath.isAbsolute(root)) throw new Error('expected an absolute failure fixture directory');
let build;
try {
  let plugin;
  if (variant === 'full-ordinary') {
    const Vue = (await import('unplugin-vue/rolldown')).default;
    plugin = Vue({ root, isProduction: true, sourceMap: false, inlineTemplate: true });
  } else if (variant === 'ordinary') {
    plugin = (await import('../../parallel-vue-plugin/impl.js')).vueTransformPlugin({ root });
  } else {
    plugin = (await import('../../parallel-vue-plugin/index.js')).default({ root });
  }
  build = await rolldown({
    cwd: root,
    input: 'invalid.vue',
    external: ['vue'],
    logLevel: 'silent',
    moduleTypes: { vue: 'js' },
    plugins: [plugin],
  });
  await build.generate({ format: 'esm' });
  throw new Error('invalid Vue SFC unexpectedly built successfully');
} catch (error) {
  if (error?.message === 'invalid Vue SFC unexpectedly built successfully') throw error;
  console.log(JSON.stringify({ variant, error: serializeError(error) }));
} finally {
  await build?.close();
}

function serializeError(error) {
  if (!error || typeof error !== 'object') return { value: String(error) };
  const serialized = {};
  for (const field of ['name', 'message', 'code', 'plugin', 'pluginCode', 'id', 'hook', 'frame']) {
    if (error[field] !== undefined) serialized[field] = error[field];
  }
  if (error.loc !== undefined) serialized.loc = error.loc;
  if (typeof error.stack === 'string') serialized.stack = error.stack;
  if (Array.isArray(error.errors)) serialized.errors = error.errors.map(serializeError);
  if (error.cause !== undefined) serialized.cause = serializeError(error.cause);
  return serialized;
}
