import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

export function getDefaultDevRuntime(host = 'localhost', port = 3000): string {
  const runtime = fs.readFileSync(
    fileURLToPath(import.meta.resolve('rolldown/experimental/runtime')),
    'utf8',
  );
  const defaultRuntime = fs.readFileSync(
    fileURLToPath(import.meta.resolve('#default-runtime')),
    'utf8',
  );
  return `${runtime}\n${defaultRuntime.replaceAll('$ADDR', `${host}:${port}`)}`;
}
