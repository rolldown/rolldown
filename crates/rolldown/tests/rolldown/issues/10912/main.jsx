import { Tag, Ns, Derived } from './tag.js';
import { CjsTag } from './cjs.cjs';

export const Local = 'span';

export const a = <Tag />;
export const b = <Ns.x />;
export const c = <Local>text</Local>;
export const d = <Derived />;
export const e = <CjsTag />;
