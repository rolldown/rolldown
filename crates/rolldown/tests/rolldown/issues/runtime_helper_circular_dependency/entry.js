// Main entry: uses CJS dep (runtime helpers go here) and statically imports gateway
import { helper } from './cjs-dep.cjs';
import { gateway } from './gateway.js';

export function main() {
  return helper() + gateway();
}
