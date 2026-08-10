// The helper name is computed at runtime, so the bundled config keeps the
// dynamic import as-is: it must resolve beside this config file when the CLI
// invokes the deferred function, after `loadConfig` removed its transient output.
export default async function deferredConfig(): Promise<Record<string, unknown>> {
  const helperName = ['config', 'helper'].join('-');
  const { input } = await import(`./${helperName}.mjs`);
  return {
    input,
    cwd: import.meta.dirname,
    output: {
      dir: 'dist',
    },
  };
}
