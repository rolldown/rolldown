/* @__NO_SIDE_EFFECTS__ */ export default function f(y) { sideEffect(y) }
f('removeThisCall')
f(onlyKeepThisIdentifier)
x(f('keepThisCall'))