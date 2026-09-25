import fs from 'node:fs';

// defined in the build step; undefined when running from source (the `dev` export condition)
declare const __RUNTIME_STRING__: string | undefined;

export function getDefaultDevRuntime(host = 'localhost', port = 3000): string {
  const runtime =
    typeof __RUNTIME_STRING__ !== 'undefined' ? __RUNTIME_STRING__ : readDefaultDevRuntimeSource();
  return runtime.replaceAll('$ADDR', `${host}:${port}`);
}

export function readDefaultDevRuntimeSource(): string {
  const read = (file: string) =>
    fs.readFileSync(
      new URL(`../../../../crates/rolldown_plugin_hmr/src/runtime/${file}`, import.meta.url),
      'utf-8',
    );
  return `${read('runtime-extra-dev-common.js')}\n${read('runtime-extra-dev-default.js')}`;
}
