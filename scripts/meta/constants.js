import * as selfExports from './constants.js';
// oxlint-disable
import { workspaceRoot } from '@rolldown/testing';

export const REPO_ROOT = workspaceRoot();

if (process.argv[1] === import.meta.filename) {
  // If this file is executed directly, print the exports
  console.log(selfExports);
}
