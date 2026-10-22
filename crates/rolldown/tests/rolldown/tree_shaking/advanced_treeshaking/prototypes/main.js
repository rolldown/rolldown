// A read of a data property of a prototype, or of an object with a `null` prototype, has no
// effect.
const nullProto = { __proto__: null, a: 'DEAD_null_proto' };
nullProto.a;
const inherits = { __proto__: { a: 'DEAD_proto_read' } };
inherits.a;

// A value of the prototype folds a branch.
const proto = { debug: false };
const child = { __proto__: proto };
export function main() {
  if (child.debug) console.log('DEAD_proto_branch');
  return 1;
}
