// Under ESM output, the finalizer prints this CJS entry inside its own CJS closure. The wrapper
// binding is synthesized, so it avoids the names of both locals (`require_main$2`), and the locals
// keep their names. Before, the renamer renamed the local `require_main` onto its sibling
// `require_main$1`.
const require_main = { value: 'a' };
const require_main$1 = { value: 'b' };

exports.pair = [require_main, require_main$1];
