import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

const [scenario, rollupModule] = process.argv.slice(2);
const build = rollupModule
  ? (await import(rollupModule)).rollup
  : (await import('rolldown')).rolldown;
const isEmitted = scenario === 'emitted';
const entry = fileURLToPath(
  new URL(isEmitted ? './emitted/main.js' : './input/main.js', import.meta.url),
);
let entryId;
const bundle = await build({
  input: isEmitted ? [] : [entry],
  plugins: [
    {
      name: 'load-in-resolve',
      buildStart() {
        if (isEmitted) this.emitFile({ type: 'chunk', id: entry });
      },
      async resolveId(source, importer, options) {
        const resolved = await this.resolve(source, importer, { ...options, skipSelf: true });
        if (resolved && !resolved.external) {
          if (options.isEntry) entryId = resolved.id;
          const info = await this.load(resolved);
          assert.ok(info.code.includes('value'));
        }
        return resolved;
      },
      load(id) {
        if (scenario === 'input-load-error' && id === entry) {
          throw new Error('fixture entry load failed');
        }
      },
      buildEnd(error) {
        if (!error) {
          assert.ok(entryId);
          assert.equal(this.getModuleInfo(entryId)?.isEntry, true);
        }
      },
    },
  ],
});
try {
  const { output } = await bundle.generate({ format: 'es' });
  const chunk = output.find((file) => file.type === 'chunk' && file.facadeModuleId === entryId);
  assert.ok(chunk);
  const { value } = await import(
    `data:text/javascript;base64,${Buffer.from(chunk.code).toString('base64')}`
  );
  console.log(value);
} finally {
  await bundle.close();
}
