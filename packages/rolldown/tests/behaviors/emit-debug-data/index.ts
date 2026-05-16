import { hello } from './hello';
import { duplicateA } from 'duplicate-a';
import { duplicateB } from 'duplicate-b';
import { metaInfo } from 'meta-info-lib';
import 'unused-lib';

console.log(hello());
console.log(duplicateA, duplicateB);
console.log(metaInfo);
void import('./async');

export default hello;
