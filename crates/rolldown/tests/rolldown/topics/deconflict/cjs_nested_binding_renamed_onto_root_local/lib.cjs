// The parameter `exports` would shadow the closure's `exports` parameter, so it is renamed. The
// new name must not be `exports$1`: that is a root-scope local of this module, which kept its
// original name and is invisible to the conflict resolver. Landing on it silently makes the
// function read its own parameter instead of the local.
const exports$1 = { tag: 'root-local' };

function read(exports) {
  return [exports.tag, exports$1.tag];
}

module.exports = { read };
