// A CJS entry renders inside its own `require_main` closure. `require_main` shadows that wrapper
// name, so deconfliction renames it, and must not land it on its sibling `require_main$1`. The entry
// module's usual "keep original names" shortcut doesn't hold here: nothing renames the closure's
// locals apart later.
const require_main = { value: 'a' };
const require_main$1 = { value: 'b' };

exports.pair = [require_main, require_main$1];
