import fs from 'node:fs';
import path from 'node:path';
import { styleText } from 'node:util';
import { x } from 'tinyexec';
import { ensureViteCheckout, viteDir } from '../../scripts/src/setup-vite/checkout.js';

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

// The tests run on a throwaway LOCAL clone of the shared `vite/` submodule,
// never on the submodule itself: this suite edits tracked files (pnpm
// overrides, spec patches) and the submodule must stay pristine. The clone
// shares objects via hardlinks (no network) and checks out the submodule's
// current HEAD — the same commit the dev-server tests run on.
printTitle('# Ensuring the vite submodule checkout...');
ensureViteCheckout();
await runCmdAndPipeOrExit(
  '# Cloning the local vite checkout...',
  ['git', ['clone', viteDir, REPO_PATH]],
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

// Skip known failing tests
// https://github.com/rolldown/rolldown/issues/8839
const assetsSpecPath = path.resolve(REPO_PATH, 'playground/assets/__tests__/assets.spec.ts');
const assetsSpec = fs.readFileSync(assetsSpecPath, 'utf-8');
fs.writeFileSync(assetsSpecPath, assetsSpec.replace(
  "test('import with raw query'",
  "test.skip('import with raw query'"
), 'utf-8');

// Rolldown keeps the deduplicated CSS file under the `style2-*` name, not
// `style-*` (same adjustment as vitejs/vite@d716106b5 on the old rolldown-canary branch).
const cssCodesplitSpecPath = path.resolve(
  REPO_PATH,
  'playground/css-codesplit/__tests__/css-codesplit-consistent.spec.ts',
);
const cssCodesplitSpec = fs.readFileSync(cssCodesplitSpecPath, 'utf-8');
fs.writeFileSync(cssCodesplitSpecPath, cssCodesplitSpec.replaceAll(
  `      expect(findAssetFile(/style2-.+\\.css/)).toBeUndefined()
      expect(findAssetFile(/style-.+\\.css/)).toMatch('h2{color:#00f}')`,
  `      expect(findAssetFile(/style-.+\\.css/)).toBeUndefined()
      expect(findAssetFile(/style2-.+\\.css/)).toMatch('h2{color:#00f}')`,
), 'utf-8');

// With client-side HMR, `import.meta.hot.invalidate()` is handled inside the
// client and never reaches the server, so there is no "hmr invalidate" server
// log anymore. Assert the user-visible result instead.
const fbmHmrSpecPath = path.resolve(
  REPO_PATH,
  'playground/hmr-full-bundle-mode/__tests__/hmr-full-bundle-mode.spec.ts',
);
const fbmHmrSpec = fs.readFileSync(fbmHmrSpecPath, 'utf-8');
fs.writeFileSync(fbmHmrSpecPath, fbmHmrSpec.replace(
  `    await expect
      .poll(() => serverLogs.slice(logIndex).join('\\n'))
      .toContain('hmr invalidate')`,
  `    await expect
      .poll(() => page.textContent('.invalidation-parent'))
      .toBe('child updated')`,
), 'utf-8');

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
