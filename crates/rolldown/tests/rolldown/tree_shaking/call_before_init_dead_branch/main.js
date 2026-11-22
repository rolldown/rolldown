if (false) inIf();
true ? 0 : inConditional();
false && inAnd();
true || inOr();
0 ?? inCoalesce();
if (DEV) inDefine();
if (0n) inBigInt();
0n ?? inBigIntCoalesce();

const inIf = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: if');
const inConditional = /* @__NO_SIDE_EFFECTS__ */ () =>
  globalThis.calls.push('removed: conditional');
const inAnd = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: and');
const inOr = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: or');
const inCoalesce = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: coalesce');
const inDefine = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: define');
const inBigInt = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: bigint');
const inBigIntCoalesce = /* @__NO_SIDE_EFFECTS__ */ () =>
  globalThis.calls.push('removed: bigint coalesce');

inIf();
inConditional();
inAnd();
inOr();
inCoalesce();
inDefine();
inBigInt();
inBigIntCoalesce();
globalThis.calls.push('done');
