// The specifier is assembled at runtime so the bundler cannot analyze or
// inline it: the import must resolve at call time, relative to the directory
// the config file lives in.
export default async function deferredConfig(): Promise<{ input: string }> {
  const helperName = ['deferred', 'helper'].join('-');
  const { input } = await import(`./${helperName}.mjs`);
  return { input };
}
