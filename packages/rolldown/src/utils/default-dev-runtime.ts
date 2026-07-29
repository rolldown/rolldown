import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

// The public entry's generated first line imports helpers for standalone ESM use. Remove it before
// appending the source to Rolldown's internal runtime module, where those helpers are already in
// scope.
export function getDefaultDevRuntime(host = 'localhost', port = 3000): string {
  const runtimeEntry = fs.readFileSync(
    fileURLToPath(import.meta.resolve('rolldown/experimental/runtime')),
    'utf8',
  );
  const runtimeHelperImportEnd = runtimeEntry.indexOf('\n');
  if (!runtimeEntry.startsWith('import ') || runtimeHelperImportEnd === -1) {
    throw new Error('Expected the standalone runtime to start with a helper import');
  }
  const runtime = runtimeEntry.slice(runtimeHelperImportEnd + 1);
  const defaultRuntime = fs.readFileSync(
    fileURLToPath(import.meta.resolve('#default-runtime')),
    'utf8',
  );
  return `${runtime}\n${defaultRuntime.replaceAll('$ADDR', `${host}:${port}`)}`;
}
