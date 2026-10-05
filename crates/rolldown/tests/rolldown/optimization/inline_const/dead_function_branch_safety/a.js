import { DEV, PROD, NIL, mutable, enable } from './env.js';
import { dead, live, mutableHelper, object } from './internal.js';
import * as internal from './internal.js';
import { effectHelper } from './effect.js';

export { enable };

export function a() {
  if ((globalThis.branchEffects.push('test'), DEV)) dead();
  if (DEV) effectHelper();
  if (PROD) {
    globalThis.branchEffects.push('live branch');
  } else {
    dead();
  }
  DEV && dead();
  PROD || internal.dead();
  PROD ?? dead();
  NIL ?? globalThis.branchEffects.push('nullish');
  globalThis.branchEffects.push(DEV ? dead() : 'ternary');
  if (mutable) return mutableHelper();
  return live();
}

export function hoisted() {
  if (DEV) {
    var value = dead();
  }
  return value;
}

export function shadowed(DEV) {
  if (DEV) return live();
  return 'shadowed';
}

export function indirectLogical() {
  return (object.method || (DEV && dead()))();
}

export function indirectConditional() {
  return (PROD ? object.method : dead)();
}

export function throwingCondition() {
  if (
    ((() => {
      throw new Error('condition');
    })(),
    DEV)
  )
    dead();
}

export function nested(outer) {
  if (outer)
    if (DEV) dead();
    else if (PROD) return 'nested';
    else return 'outer';
  return 'neither';
}

export function lexical() {
  const value = 'outer';
  if (PROD) {
    const value = 'inner';
    globalThis.branchEffects.push(value);
  } else dead();
  return value;
}
