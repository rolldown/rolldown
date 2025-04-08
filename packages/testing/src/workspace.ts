import nodeAssert from 'node:assert';
import nodeFs from 'node:fs';
import nodePath from 'node:path';

/**
 * @description
 * - Get the absolute path to the root of the workspace. The root is always the directory containing the root `Cargo.toml`, `package.json`, `pnpm-workspace.yaml` etc.
 * - `workspaceRoot('packages')` equals to `path.resolve(workspaceRoot(), 'packages')`
 */
export function workspaceRoot(...joined: string[]) {
  return nodePath.resolve(import.meta.dirname, '../../..', ...joined);
}

nodeAssert(
  nodeFs.existsSync(workspaceRoot('pnpm-workspace.yaml')),
  `${workspaceRoot('pnpm-workspace.yaml')} does not exist`,
);
