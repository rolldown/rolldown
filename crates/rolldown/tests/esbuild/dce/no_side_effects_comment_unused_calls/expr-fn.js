const f = /* @__NO_SIDE_EFFECTS__ */ function (y) { sideEffect(y) }
const g = /* @__NO_SIDE_EFFECTS__ */ function* (y) { sideEffect(y) }
f('removeThisCall')
g('removeThisCall')
f(onlyKeepThisIdentifier)
g(onlyKeepThisIdentifier)
x(f('keepThisCall'))
x(g('keepThisCall'))