import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repository = fileURLToPath(new URL('../../', import.meta.url));
const buildArguments = [
  'build',
  '--locked',
  '--release',
  '-p',
  'bench',
  '--features',
  'link-baseline',
  '--bin',
  'link-baseline',
];
const buildCommand = `cargo ${buildArguments.join(' ')}`;

function output(program, args) {
  return execFileSync(program, args, { cwd: repository, encoding: 'utf8' }).trim();
}

function rustcField(verbose, name) {
  const prefix = `${name}: `;
  const line = verbose.split('\n').find((candidate) => candidate.startsWith(prefix));
  if (!line) {
    throw new Error(`rustc -vV did not report ${name}`);
  }
  return line.slice(prefix.length);
}

for (const variable of [
  'RUSTC_WRAPPER',
  'RUSTC_WORKSPACE_WRAPPER',
  'RUSTFLAGS',
  'CARGO_ENCODED_RUSTFLAGS',
]) {
  if (process.env[variable]) {
    throw new Error(`${variable} must be unset for a canonical link baseline build`);
  }
}

const rustcVerbose = output('rustc', ['-vV']);
const gitStatus = output('git', ['status', '--porcelain=v1', '--untracked-files=normal']);
const environment = {
  ...process.env,
  RUSTFLAGS: '',
  CARGO_BUILD_RUSTFLAGS: '',
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS: '',
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS: '1',
  CARGO_PROFILE_RELEASE_DEBUG: 'false',
  CARGO_PROFILE_RELEASE_INCREMENTAL: 'false',
  CARGO_PROFILE_RELEASE_LTO: 'fat',
  CARGO_PROFILE_RELEASE_OPT_LEVEL: '3',
  CARGO_PROFILE_RELEASE_STRIP: 'symbols',
  ROLLDOWN_LINK_BASELINE_BUILD_PROVENANCE_VERSION: '1',
  ROLLDOWN_LINK_BASELINE_BUILD_GIT_COMMIT: output('git', ['rev-parse', 'HEAD']),
  ROLLDOWN_LINK_BASELINE_BUILD_GIT_TREE: output('git', ['rev-parse', 'HEAD^{tree}']),
  ROLLDOWN_LINK_BASELINE_BUILD_GIT_DIRTY: String(gitStatus.length > 0),
  ROLLDOWN_LINK_BASELINE_BUILD_RUSTC: output('rustc', ['--version']),
  ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_COMMIT_HASH: rustcField(rustcVerbose, 'commit-hash'),
  ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_HOST: rustcField(rustcVerbose, 'host'),
  ROLLDOWN_LINK_BASELINE_BUILD_RUSTC_LLVM: rustcField(rustcVerbose, 'LLVM version'),
  ROLLDOWN_LINK_BASELINE_BUILD_CARGO: output('cargo', ['--version']),
  ROLLDOWN_LINK_BASELINE_BUILD_LTO: 'fat',
  ROLLDOWN_LINK_BASELINE_BUILD_CODEGEN_UNITS: '1',
  ROLLDOWN_LINK_BASELINE_BUILD_STRIP: 'symbols',
  ROLLDOWN_LINK_BASELINE_BUILD_COMMAND: buildCommand,
};

const result = spawnSync('cargo', buildArguments, {
  cwd: repository,
  env: environment,
  stdio: 'inherit',
});
if (result.error) {
  throw result.error;
}
process.exitCode = result.status ?? 1;
