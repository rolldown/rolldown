// Both locals render inside this module's closure. `require_dup` matches a captured chunk-scope
// name, so deconfliction renames it, and must not land it on its sibling `require_dup$1`, which
// kept its original name and is invisible to the conflict resolver. This is the shape rolldown's
// own CJS output has, so re-bundling a rolldown-built package hits it.
const require_dup = { value: 'a' };
const require_dup$1 = { value: 'b' };

exports.pair = [require_dup, require_dup$1];
