import * as ns from './shared.js';

// Eliminated: direct namespace-member call to a `@__NO_SIDE_EFFECTS__` function.
ns.fn();
// Eliminated: computed namespace-member access resolves the same way.
ns['fn']();
// Eliminated: the function-expression form is annotated and handled identically.
ns.fnExpr();

// Preserved: the result is used, so the call cannot be dropped.
const used = ns.fn();
console.log(used);

// Preserved: `ns.fn.call(...)` is not a direct call to `fn` (trailing property
// access), so the annotation must not apply.
ns.fn.call(null);
