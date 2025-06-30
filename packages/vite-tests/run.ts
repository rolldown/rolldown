import fs from 'node:fs';
import path from 'node:path';
import colors from 'picocolors';
import { x } from 'tinyexec';

const REPO_PATH = path.resolve(import.meta.dirname, './repo');
const OVERRIDES = [
  `  rolldown: ${path.resolve(import.meta.dirname, '../rolldown')}`,
  `  "@rolldown/pluginutils": ${path.resolve(import.meta.dirname, '../pluginutils')}`
];

function printTitle(title: string) {
  console.info(colors.cyan(colors.bold(title)));
}

async function runCmdAndPipe(title: string, cmdOptions: Parameters<typeof x>) {
  printTitle(title);
  console.info('------------------------');
  const proc = x(...cmdOptions);
  proc.process?.stdout?.pipe(process.stdout);
  proc.process?.stderr?.pipe(process.stderr);
  const result = await proc;
  console.info('------------------------');
  if (result.exitCode !== 0) {
    console.error(
      colors.red(
        `${colors.bold('Failed to execute command:')} ${
          [cmdOptions[0]].concat(cmdOptions[1] ?? []).join(' ')
        }`,
      ),
    );
    process.exit(1);
  }
  return result;
}

fs.rmSync(REPO_PATH, { recursive: true, force: true });

await runCmdAndPipe(
  '# Cloning rolldown-vite repo...',
  ['git', ['clone', 'https://github.com/vitejs/rolldown-vite.git', REPO_PATH]],
);

printTitle('# Updating pnpm-workspace.yaml to link to local rolldown...');
const pnpmWorkspace = path.resolve(REPO_PATH, 'pnpm-workspace.yaml');
const pnpmWorkspaceYaml = fs.readFileSync(pnpmWorkspace, 'utf-8');
const newPnpmWorkspaceYaml = pnpmWorkspaceYaml.replace(
  /overrides:\n/,
  `overrides:\n${OVERRIDES.join('\n')}\n`
);
fs.writeFileSync(pnpmWorkspace, newPnpmWorkspaceYaml, 'utf-8');

await runCmdAndPipe(
  '# Running `pnpm install`...',
  ['pnpm', ['install', '--no-frozen-lockfile'], { nodeOptions: { cwd: REPO_PATH } }],
);
await runCmdAndPipe(
  '# Running `pnpm run build`...',
  ['pnpm', ['run', 'build'], { nodeOptions: { cwd: REPO_PATH } }],
);
await runCmdAndPipe(
  '# Running `pnpm test`...',
  ['pnpm', ['run', 'test'], { nodeOptions: { cwd: REPO_PATH } }],
);
await runCmdAndPipe(
  '# Running `_VITE_TEST_NATIVE_PLUGIN=1 pnpm test`...',
  ['pnpm', ['run', 'test'], {
    nodeOptions: {
      cwd: REPO_PATH,
      env: { _VITE_TEST_NATIVE_PLUGIN: '1' },
    },
  }],
);
