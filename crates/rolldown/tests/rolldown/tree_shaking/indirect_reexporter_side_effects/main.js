import { foo as fooIndirect } from './reexporter-indirect.js';
import { foo as fooDirect } from './reexporter-direct.js';
import { foo as fooChained } from './reexporter-chain-1.js';
import { foo as fooIndirectIgnored } from './reexporter-indirect-ignored.js';
import { foo as fooPure } from './reexporter-pure.js';
import * as namespace from './reexporter-namespace.js';

export const observed = {
  fooIndirect,
  fooDirect,
  fooChained,
  fooPure,
  namespaceFoo: namespace.namespaceFoo,
};
