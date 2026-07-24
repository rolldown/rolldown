import fs from 'node:fs';
import path from 'node:path';
import { styleText } from 'node:util';
import { x } from 'tinyexec';

const VITE_DIR = path.resolve(import.meta.dirname, '../../vite');
const REPO_PATH = path.resolve(import.meta.dirname, './repo');
const OVERRIDES = [
  `  rolldown: ${path.resolve(import.meta.dirname, '../rolldown')}`,
  `  "@rolldown/pluginutils": ${path.resolve(import.meta.dirname, '../pluginutils')}`
];

function printTitle(title: string) {
  console.info(styleText(['cyan', 'bold'], title));
}

async function runCmdAndPipe(title: string, cmdOptions: Parameters<typeof x>): Promise<boolean> {
  printTitle(title);
  console.info('------------------------');
  const proc = x(...cmdOptions);
  proc.process?.stdout?.pipe(process.stdout);
  proc.process?.stderr?.pipe(process.stderr);
  const result = await proc;
  console.info('------------------------');
  if (result.exitCode !== 0) {
    console.error(
      styleText(
        'red',
        `${styleText('bold', 'Failed to execute command:')} ${
          [cmdOptions[0]].concat(cmdOptions[1] ?? []).join(' ')
        }`,
      ),
    );
    return true;
  }
  return false;
}

async function runCmdAndPipeOrExit(title: string, cmdOptions: Parameters<typeof x>): Promise<void> {
  const failed = await runCmdAndPipe(title, cmdOptions);
  if (failed) {
    process.exit(1);
  }
}

fs.rmSync(REPO_PATH, { recursive: true, force: true });

// Reuse the shared `vite/` checkout at the repo root, prepared by
// `just setup-vite` (the latest `rolldown-canary` rebased onto the latest
// `main`), the same code the dev-server tests run on. Setup happens only
// there, never here, so the checkout and the dev-server's built vite dist
// cannot drift apart. The tests run on a throwaway LOCAL clone of the
// checkout, never on the checkout itself: this suite edits tracked files
// (pnpm overrides) and the checkout must stay unpatched. The clone shares
// objects via hardlinks, so it needs no network.
if (!fs.existsSync(path.join(VITE_DIR, 'package.json'))) {
  console.error(
    styleText(['red', 'bold'], `Vite checkout not found at ${VITE_DIR}. Run \`just setup-vite\` first.`),
  );
  process.exit(1);
}
await runCmdAndPipeOrExit(
  '# Cloning the local vite checkout...',
  ['git', ['clone', VITE_DIR, REPO_PATH]],
);

printTitle('# Updating pnpm-workspace.yaml to link to local rolldown...');
const pnpmWorkspace = path.resolve(REPO_PATH, 'pnpm-workspace.yaml');
const pnpmWorkspaceYaml = fs.readFileSync(pnpmWorkspace, 'utf-8');
const newPnpmWorkspaceYaml = pnpmWorkspaceYaml.replace(
  /overrides:\n\s*rolldown:\s*\$rolldown\n/,
  `overrides:\n${OVERRIDES.join('\n')}\n`
);
fs.writeFileSync(pnpmWorkspace, newPnpmWorkspaceYaml, 'utf-8');

await runCmdAndPipeOrExit(
  '# Running `pnpm install`...',
  ['pnpm', ['install', '--no-frozen-lockfile'], { nodeOptions: { cwd: REPO_PATH } }],
);
await runCmdAndPipeOrExit(
  '# Running `pnpm exec playwright install chromium`...',
  ['pnpm', ['exec', 'playwright', 'install', 'chromium'], { nodeOptions: { cwd: REPO_PATH } }],
);
await runCmdAndPipeOrExit(
  '# Running `pnpm run build`...',
  ['pnpm', ['run', 'build'], { nodeOptions: { cwd: REPO_PATH } }],
);

// Remove VITE_PLUS_* env vars to prevent leaking into loadEnv() test snapshots
for (const key of Object.keys(process.env)) {
  if (key.startsWith('VITE_PLUS_')) {
    delete process.env[key];
  }
}

const failed = []

const failedTestUnit = await runCmdAndPipe(
  '# Running `pnpm test-unit`...',
  ['pnpm', ['run', 'test-unit'], { nodeOptions: { cwd: REPO_PATH } }],
);
if (failedTestUnit) failed.push('test-unit');

const failedTestServe = await runCmdAndPipe(
  '# Running `pnpm test-serve`...',
  ['pnpm', ['run', 'test-serve'], { nodeOptions: { cwd: REPO_PATH } }],
);
if (failedTestServe) failed.push('test-serve');

const failedTestServeBundled = await runCmdAndPipe(
  '# Running `pnpm test-serve-bundled`...',
  ['pnpm', ['run', 'test-serve-bundled'], { nodeOptions: { cwd: REPO_PATH } }],
);
if (failedTestServeBundled) failed.push('test-serve-bundled');

const failedTestBuild = await runCmdAndPipe(
  '# Running `pnpm test-build`...',
  ['pnpm', ['run', 'test-build'], { nodeOptions: { cwd: REPO_PATH } }],
);
if (failedTestBuild) failed.push('test-build');

if (failed.length > 0) {
  console.error(styleText(['red', 'bold'], 'The following test suites failed:'));
  failed.forEach(test => console.error(styleText('red', ` - ${test}`)));
  process.exit(1);
}
