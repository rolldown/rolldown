// Under CJS output, `join` is read through the external's namespace binding (`node_path.join`),
// even here, where `node:path` is only reached through `reexport.js`. A parameter named
// `node_path` must not capture it.
import { join } from './reexport.js';

export function read(node_path) {
  return [typeof join, node_path];
}
