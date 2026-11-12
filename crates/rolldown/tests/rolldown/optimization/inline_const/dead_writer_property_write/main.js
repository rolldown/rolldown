import { flag, run } from './flag.js';
import { imported, run_imported } from './imported-flag.js';
import { unused } from './unused-flag.js';

export function read() {
  run();
  run_imported();
  return [flag, imported, unused];
}
