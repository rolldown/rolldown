import assert from 'node:assert/strict';

import aliased from './aliased.json';
import argument from './argument.json';

// These are the JSON-default escapes that the merged member-write bailout (#9972) still
// optimizes incorrectly: the object leaves as a bare reference (not a static `data.key` access),
// per-key split export would diverge from the live, mutated object. The mutation/computed-write,
// named-re-export, cross-module and read-only cases are already covered by `issues/9484*` and
// `tree_shaking/json_default_import`, so this fixture only needs the escape forms.

// Escape via alias: the object flows into `alias`, which then mutates it. `aliased.v` must read
// the live (mutated) object, not a stale split export.
const alias = aliased;
alias.v = 'after';
assert.strictEqual(aliased.v, 'after');

// Escape via call argument: the object is handed to a function that mutates it.
function mutate(object) {
  object.v = 'after';
}
mutate(argument);
assert.strictEqual(argument.v, 'after');
