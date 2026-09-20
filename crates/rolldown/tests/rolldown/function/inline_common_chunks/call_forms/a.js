import * as ns from './shared.js';
import { f, tag, K, obj } from './shared.js';
globalThis.events.push(
  [
    f(),
    f?.(),
    ns.f(),
    ns.f?.(),
    tag`x`,
    ns.tag`x`,
    new K().ok,
    new ns.K().ok,
    obj.m(),
    ns.obj.m(),
  ].join(),
);
