import { expect, inject } from 'vitest';

export interface Container {
  runCommand: (cmd: string, args: string[]) => PromiseLike<string>;
  readdir: (path: string) => Promise<string[]>;
  readFile: (path: string) => Promise<string>;
}

// `error` (not `true`) so a missing WASI binding fails instead of silently falling back
export const FORCE_WASI = 'NAPI_RS_FORCE_WASI=error';

// runCommand does not surface an exit code, so the shell echoes it into the output.
// The last marker wins, so output that happens to contain the token cannot mask a failure.
export async function run(container: Container, script: string) {
  const output = await container.runCommand('sh', ['-c', `${script}; echo "__exit=$?"`]);
  const markers = [...output.matchAll(/__exit=(\d+)/g)];
  const exitCode = Number(markers.at(-1)?.[1] ?? NaN);
  return { output, exitCode };
}

// The tarballs are gitignored build output, so a stale one from an earlier build would
// otherwise pass silently. Pin the version actually running inside the container.
export async function expectFreshArtifact(container: Container) {
  // safe for both scenarios: @rolldown/browser has no napi loader, so the variable is inert there
  const version = await run(container, `${FORCE_WASI} pnpm exec rolldown --version`);
  expect(version, version.output).toMatchObject({ exitCode: 0 });
  expect(version.output).toContain(inject('rolldownVersion'));
  expectNoRegistryFallback(version.output);
}

// packages/rolldown/src/webcontainer-fallback.cjs downloads @rolldown/binding-wasm32-wasi from
// npm when the binding is missing inside a WebContainer, which would test the released artifact
// instead of the packed one
export function expectNoRegistryFallback(output: string) {
  expect(output).not.toContain('[rolldown] Downloading');
}

// mount() snapshots the fixture directory as-is, so a dist/ left behind on the host would
// be mounted and could satisfy the output assertions without the container building anything
export async function expectNoPreexistingDist(container: Container) {
  const listing = await run(container, 'ls dist');
  expect(listing.exitCode, `dist/ existed before the build:\n${listing.output}`).not.toBe(0);
}
