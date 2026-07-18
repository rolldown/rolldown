import {
  isolatedDeclaration as originalIsolatedDeclaration,
  isolatedDeclarationSync,
} from '../binding.cjs';
import { leaseAsyncFunction } from './run-with-runtime-lease';

export { isolatedDeclarationSync };

export const isolatedDeclaration: typeof originalIsolatedDeclaration = leaseAsyncFunction(
  originalIsolatedDeclaration,
  'Isolated declaration generation and runtime release both failed',
);
