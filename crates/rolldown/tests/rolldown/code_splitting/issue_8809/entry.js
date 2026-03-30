import { helper } from './cjs-dep.cjs';
import { gateway } from './gateway.js';
export function main() {
  return helper() + gateway();
}
