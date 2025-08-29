import { workspaceRoot } from 'rolldown-tests/utils';
import * as selfExports from './constants.js';

export const REPO_ROOT = workspaceRoot();

if (process.argv[1] === import.meta.filename) {
  // If this file is executed directly, print the exports
  console.log(selfExports);
}
