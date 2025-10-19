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
  '# Cloning rolldown-vite repo...',
  ['git', ['clone', 'https://github.com/vitejs/rolldown-vite.git', REPO_PATH]],
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
  '# Running `pnpm run build`...',
  ['pnpm', ['run', 'build'], { nodeOptions: { cwd: REPO_PATH } }],
);

const failed = []

const failedNormalTestUnit = await runCmdAndPipe(
  '# Running `pnpm test-unit`...',
  ['pnpm', ['run', 'test-unit'], { nodeOptions: { cwd: REPO_PATH } }],
);
if (failedNormalTestUnit) failed.push('test-unit');

const failedNormalTestServe = await runCmdAndPipe(
  '# Running `pnpm test-serve`...',
  ['pnpm', ['run', 'test-serve'], { nodeOptions: { cwd: REPO_PATH } }],
);
if (failedNormalTestServe) failed.push('test-serve');

const failedNormalTestBuild = await runCmdAndPipe(
  '# Running `pnpm test-build`...',
  ['pnpm', ['run', 'test-build'], { nodeOptions: { cwd: REPO_PATH } }],
);
if (failedNormalTestBuild) failed.push('test-build');

const failedJsTestUnit = await runCmdAndPipe(
  '# Running `_VITE_TEST_JS_PLUGIN=1 pnpm test-unit`...',
  ['pnpm', ['run', 'test-unit'], { nodeOptions: {
    cwd: REPO_PATH,
    env: { _VITE_TEST_JS_PLUGIN: '1' },
  } }],
);
if (failedJsTestUnit) failed.push('[JS] test-unit');
const failedJsTestServe = await runCmdAndPipe(
  '# Running `_VITE_TEST_JS_PLUGIN=1 pnpm test-serve`...',
  ['pnpm', ['run', 'test-serve'], { nodeOptions: {
    cwd: REPO_PATH,
    env: { _VITE_TEST_JS_PLUGIN: '1' },
  } }],
);
if (failedJsTestServe) failed.push('[JS] test-serve');
const failedJsTestBuild = await runCmdAndPipe(
  '# Running `_VITE_TEST_JS_PLUGIN=1 pnpm test-build`...',
  ['pnpm', ['run', 'test-build'], { nodeOptions: {
    cwd: REPO_PATH,
    env: { _VITE_TEST_JS_PLUGIN: '1' },
  } }],
);
if (failedJsTestBuild) failed.push('[JS] test-build');

if (failed.length > 0) {
  console.error(styleText(['red', 'bold'], 'The following test suites failed:'));
  failed.forEach(test => console.error(styleText('red', ` - ${test}`)));
  process.exit(1);
}
