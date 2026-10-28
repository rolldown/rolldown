// Both locals are printed inside the CJS closure of this module, and they keep their names. The
// wrapper binding of this module is synthesized, so it avoids both names (`require_dup$2`). Before,
// the renamer renamed the local `require_dup` onto its sibling `require_dup$1`, which the conflict
// resolver did not see. Rolldown's own CJS output has this shape, so a bundle of a package that
// rolldown built hits it.
const require_dup = { value: 'a' };
const require_dup$1 = { value: 'b' };

exports.pair = [require_dup, require_dup$1];
