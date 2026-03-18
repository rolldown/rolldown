import fs from 'node:fs';
import path from 'node:path';
import { styleText } from 'node:util';
import { x } from 'tinyexec';

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

await runCmdAndPipeOrExit(
  '# Cloning vite repo (rolldown-canary branch)...',
  ['git', ['clone', '--branch', 'rolldown-canary', 'https://github.com/vitejs/vite.git', REPO_PATH]],
);

await runCmdAndPipeOrExit(
  '# Rebasing rolldown-canary onto main...',
  ['git', ['rebase', 'origin/main'], { nodeOptions: { cwd: REPO_PATH } }],
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
