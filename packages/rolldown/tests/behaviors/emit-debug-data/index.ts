import { hello } from './hello';
import { duplicateA } from 'duplicate-a';
import { duplicateB } from 'duplicate-b';
import { directGraphValue } from 'direct-graph-lib';
import { metaInfo } from 'meta-info-lib';
import { unusedValue } from 'unused-lib';

console.log(hello());
console.log(duplicateA, duplicateB);
console.log(directGraphValue);
console.log(metaInfo);
void unusedValue;
void import('./async');

export default hello;
