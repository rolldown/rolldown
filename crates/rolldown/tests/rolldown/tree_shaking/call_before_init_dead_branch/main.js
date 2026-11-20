if (false) inIf();
true ? 0 : inConditional();
false && inAnd();
true || inOr();
0 ?? inCoalesce();
if (DEV) inDefine();

const inIf = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: if');
const inConditional = /* @__NO_SIDE_EFFECTS__ */ () =>
  globalThis.calls.push('removed: conditional');
const inAnd = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: and');
const inOr = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: or');
const inCoalesce = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: coalesce');
const inDefine = /* @__NO_SIDE_EFFECTS__ */ () => globalThis.calls.push('removed: define');

inIf();
inConditional();
inAnd();
inOr();
inCoalesce();
inDefine();
globalThis.calls.push('done');
