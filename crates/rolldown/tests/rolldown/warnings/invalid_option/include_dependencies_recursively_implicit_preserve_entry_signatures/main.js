import { sharedValue } from './shared.js';

console.log('Main entry with shared:', sharedValue);

export function mainFunc() {
  return 'main function';
}