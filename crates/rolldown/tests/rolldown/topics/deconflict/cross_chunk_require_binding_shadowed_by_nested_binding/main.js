// Under CJS output, `shared` lives in a chunk both entries require, and is read here as
// `require_shared.shared()`. A parameter named `require_shared` must not capture that binding.
import { shared } from './shared.js';

export function read(require_shared) {
  return [shared(), require_shared];
}
