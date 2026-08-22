import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { rolldown } from 'rolldown';

const root = mkdtempSync(path.join(tmpdir(), 'rolldown-devtools-rd-log-'));
const input = path.join(root, 'main.js');
const outputRoot = path.join(root, 'node_modules', '.rolldown');
const sessionId = 'rd-log-devtools';
writeFileSync(input, 'export const value = 1;\n');

try {
  const untraced = await rolldown({ cwd: root, input: './main.js' });
  await untraced.generate();
  await untraced.close();
  assert.equal(existsSync(outputRoot), false);

  const traced = await rolldown({
    cwd: root,
    devtools: { sessionId },
    input: './main.js',
  });
  await traced.generate();
  await traced.close();

  const logs = readFileSync(path.join(outputRoot, sessionId, 'logs.json'), 'utf8');
  assert.match(logs, /"action":"BuildEnd"/);

  const afterClose = await rolldown({ cwd: root, input: './main.js' });
  await afterClose.generate();
  await afterClose.close();
  assert.equal(existsSync(path.join(outputRoot, 'unknown_session')), false);

  console.log(
    JSON.stringify({
      isolatedOptIn: true,
      rdLogCompatible: true,
      untracedFirstThenTraced: true,
    }),
  );
} finally {
  rmSync(root, { force: true, recursive: true });
}
