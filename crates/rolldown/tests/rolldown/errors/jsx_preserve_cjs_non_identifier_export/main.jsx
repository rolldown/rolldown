import { 'my-tag' as MyTag } from './cjs.cjs';
import { 'ext-tag' as ExtTag } from 'ext';

export const a = <MyTag />;
export const b = <MyTag.Item />;
export const c = <MyTag>text</MyTag>;
export const d = <ExtTag />;
