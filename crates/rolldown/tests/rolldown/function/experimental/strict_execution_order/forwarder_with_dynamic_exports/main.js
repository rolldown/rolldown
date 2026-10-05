import { hasExpectedValue, isEqual } from './utilities/index.js';
import { flag, isEqual as isEqualViaUser } from './user.js';

globalThis.__app = [
  hasExpectedValue('expected'),
  isEqual({ key: 1 }, 'key'),
  flag,
  isEqualViaUser({ key: 1 }, 'key'),
];
