import assert from 'node:assert/strict';

namespace N {
  export enum E {
    A,
  }
}
const original = N.E;
namespace N {
  export enum E {
    B = 1,
  }
}
assert.strictEqual(N.E, original);
assert.deepStrictEqual(N.E, { 0: 'A', 1: 'B', A: 0, B: 1 });

namespace Outer.Inner {
  export enum E {
    A,
  }
}
const nested = Outer.Inner.E;
namespace Outer.Inner {
  export enum E {
    B = 1,
  }
}
assert.strictEqual(Outer.Inner.E, nested);
assert.deepStrictEqual(Outer.Inner.E, { 0: 'A', 1: 'B', A: 0, B: 1 });
