import { hello } from './hello';
import { metaInfo } from 'meta-info-lib';

console.log(hello());
console.log(metaInfo);
void import('./async');

export default hello;
