import { value } from './common-child';
import { value as parentNonBoundaryChildValue } from './parent-non-executed-child';
console.log(value, parentNonBoundaryChildValue);

globalThis.records.push('parent-non-executed');
