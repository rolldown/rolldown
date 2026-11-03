import { isSame } from './shared-identity.js';
import { hasExpectedValue, isEqual } from './utilities/index.js';

export function showToast(value) {
  if (
    hasExpectedValue(value) &&
    isEqual({ [value]: true }, 'expected') &&
    isSame(value, 'expected')
  ) {
    console.log('The expected value was received');
  }
}
