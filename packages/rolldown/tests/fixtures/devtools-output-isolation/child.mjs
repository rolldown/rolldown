import assert from 'node:assert/strict';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { rolldown } from 'rolldown';

const root = mkdtempSync(path.join(tmpdir(), 'rolldown-devtools-output-'));
const sharedSessionId = 'shared-devtools-session';
const escapingSessionId = '../../../escaped-devtools-session';
const largeSource = `export const value = ${JSON.stringify('x'.repeat(12 * 1024))};\n`;
const smallSource = 'export const value = 1;\n';
const windowsReservedNames = new Set([
  'CON',
  'PRN',
  'AUX',
  'NUL',
  ...Array.from({ length: 9 }, (_, index) => `COM${index + 1}`),
  ...Array.from({ length: 9 }, (_, index) => `LPT${index + 1}`),
]);

function createProject(name, source = largeSource) {
  const cwd = path.join(root, name);
  mkdirSync(cwd, { recursive: true });
  writeFileSync(path.join(cwd, 'main.js'), source);
  return cwd;
}

function createBlockedProject(name) {
  const cwd = createProject(name);
  mkdirSync(path.join(cwd, 'node_modules'), { recursive: true });
  writeFileSync(outputRoot(cwd), 'blocks the devtools output directory');
  return cwd;
}

function outputRoot(cwd) {
  return path.join(cwd, 'node_modules', '.rolldown');
}

function readEvents(filename) {
  return readFileSync(filename, 'utf8')
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line));
}

function formatError(error) {
  if (error instanceof AggregateError) {
    return `${error.message}: ${error.errors.map(formatError).join(' | ')}`;
  }
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

function assertSelfContainedStringRefs(filename, expectedSessionId) {
  const events = readEvents(filename);
  const stringRefs = new Set(
    events.filter((event) => event.action === 'StringRef').map((event) => event.id),
  );
  const referencedHashes = events.flatMap((event) =>
    Object.values(event).flatMap((value) =>
      typeof value === 'string' && value.startsWith('$ref:') ? [value.slice('$ref:'.length)] : [],
    ),
  );

  assert(referencedHashes.length > 0, `${filename} must emit a large-string reference`);
  for (const hash of referencedHashes) {
    assert(stringRefs.has(hash), `missing local StringRef ${hash} in ${filename}`);
  }
  for (const event of events) {
    if (event.action !== 'StringRef') {
      assert.equal(event.session_id, expectedSessionId);
    }
  }
}

function assertSessionEvents(cwd, sessionDirectory, expectedSessionId) {
  for (const event of readEvents(path.join(outputRoot(cwd), sessionDirectory, 'logs.json'))) {
    if (event.action !== 'StringRef') {
      assert.equal(event.session_id, expectedSessionId);
    }
  }
}

function expectedPlainOrHexDirectory(sessionId) {
  const bytes = Buffer.from(sessionId);
  const portable =
    bytes.length > 0 &&
    bytes.length <= 200 &&
    /^[a-z0-9_-]+$/.test(sessionId) &&
    !windowsReservedNames.has(sessionId.toUpperCase());
  if (portable) {
    return sessionId;
  }
  if (bytes.length <= 95) {
    return `~${bytes.toString('hex')}`;
  }
  return null;
}

function assertEncodedSessionDirectory(cwd, sessionId) {
  const directories = readdirSync(outputRoot(cwd));
  assert.equal(directories.length, 1);
  const expected = expectedPlainOrHexDirectory(sessionId);
  if (expected === null) {
    assert.match(directories[0], /^~h[0-9a-f]{64}$/);
  } else {
    assert.equal(directories[0], expected);
  }
  assertSessionEvents(cwd, directories[0], sessionId);
}

async function createBuild(cwd, sessionId, input = './main.js') {
  return rolldown({
    cwd,
    devtools: { sessionId },
    input,
    plugins: [
      {
        name: 'devtools-output-isolation',
        transform(code) {
          return code;
        },
      },
    ],
  });
}

async function closeError(build) {
  return build.close().then(
    () => null,
    (error) => error,
  );
}

try {
  const firstCwd = createProject('first');
  const secondCwd = createProject('second');
  const firstBuild = await createBuild(firstCwd, sharedSessionId);
  const secondBuild = await createBuild(secondCwd, sharedSessionId);

  await firstBuild.generate({ dir: largeSource });
  await secondBuild.generate({ dir: largeSource });
  await firstBuild.close();
  await secondBuild.close();

  assert.deepEqual(readdirSync(outputRoot(firstCwd)), [sharedSessionId]);
  assert.deepEqual(readdirSync(outputRoot(secondCwd)), [sharedSessionId]);
  for (const cwd of [firstCwd, secondCwd]) {
    const sessionRoot = path.join(outputRoot(cwd), sharedSessionId);
    assertSelfContainedStringRefs(path.join(sessionRoot, 'meta.json'), sharedSessionId);
    assertSelfContainedStringRefs(path.join(sessionRoot, 'logs.json'), sharedSessionId);
  }

  const blockedCwd = createBlockedProject('blocked');
  const blockedFirst = await createBuild(blockedCwd, sharedSessionId);
  const blockedSecond = await createBuild(blockedCwd, sharedSessionId);
  await blockedFirst.generate();
  await blockedSecond.generate();
  const [blockedFirstError, blockedSecondError] = await Promise.all([
    closeError(blockedFirst),
    closeError(blockedSecond),
  ]);
  for (const error of [blockedFirstError, blockedSecondError]) {
    assert(error instanceof AggregateError);
    assert.match(formatError(error), /devtools session flush failed/i);
  }

  const escapingCwd = createProject('escaping');
  const escapedDestination = path.join(root, 'escaped-devtools-session');
  const escapingBuild = await createBuild(escapingCwd, escapingSessionId);
  await escapingBuild.generate();
  await escapingBuild.close();

  assert.equal(existsSync(escapedDestination), false);
  assertEncodedSessionDirectory(escapingCwd, escapingSessionId);

  const boundarySessionIds = [
    '',
    '会话',
    'con',
    'A'.repeat(95),
    'A'.repeat(96),
    'x'.repeat(200),
    'x'.repeat(201),
    'z'.repeat(12 * 1024),
  ];
  for (const [index, sessionId] of boundarySessionIds.entries()) {
    const cwd = createProject(`session-boundary-${index}`, smallSource);
    const build = await createBuild(cwd, sessionId);
    await build.generate();
    await build.close();
    assertEncodedSessionDirectory(cwd, sessionId);
  }

  let canonicalAliases = false;
  if (process.platform !== 'win32') {
    const realCwd = createProject('canonical-real', smallSource);
    const aliasCwd = path.join(root, 'canonical-alias');
    symlinkSync(realCwd, aliasCwd, 'dir');
    const realBuild = await createBuild(realCwd, 'canonical-session');
    const aliasBuild = await createBuild(aliasCwd, 'canonical-session');
    await realBuild.generate();
    await aliasBuild.generate();
    await realBuild.close();
    await aliasBuild.close();
    assert.deepEqual(readdirSync(outputRoot(realCwd)), ['canonical-session']);

    const parentTarget = path.join(root, 'canonical-parent-target');
    const nestedTarget = path.join(parentTarget, 'nested');
    mkdirSync(nestedTarget, { recursive: true });
    writeFileSync(path.join(parentTarget, 'main.js'), smallSource);
    const parentAlias = path.join(root, 'canonical-parent-alias');
    symlinkSync(nestedTarget, parentAlias, 'dir');
    const parentBuild = await createBuild(
      `${parentAlias}${path.sep}..`,
      'parent-session',
      path.join(parentTarget, 'main.js'),
    );
    await parentBuild.generate();
    await parentBuild.close();
    assert.deepEqual(readdirSync(outputRoot(parentTarget)), ['parent-session']);
    assert.equal(existsSync(outputRoot(root)), false);
    canonicalAliases = true;
  }

  console.log(
    JSON.stringify({
      canonicalAliases,
      encodedIdBoundaries: true,
      escapedSessionContained: true,
      independentSameKeyOwners: true,
      isolatedOutputRoots: true,
      selfContainedStringRefs: true,
    }),
  );
} finally {
  rmSync(root, { force: true, recursive: true });
}
