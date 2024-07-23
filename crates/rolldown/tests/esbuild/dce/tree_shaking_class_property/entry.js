let remove1 = class { x }
let remove2 = class { x = x }
let remove3 = class { x() {} }
let remove4 = class { get x() {} }
let remove5 = class { set x(_) {} }
let remove6 = class { async x() {} }
let remove7 = class { ['x'] = x }
let remove8 = class { ['x']() {} }
let remove9 = class { get ['x']() {} }
let remove10 = class { set ['x'](_) {} }
let remove11 = class { async ['x']() {} }
let remove12 = class { [0] = 'x' }
let remove13 = class { [null] = 'x' }
let remove14 = class { [undefined] = 'x' }
let remove15 = class { [false] = 'x' }
let remove16 = class { [0n] = 'x' }
let remove17 = class { toString() {} }

let keep1 = class { [x] = 'x' }
let keep2 = class { [x]() {} }
let keep3 = class { get [x]() {} }
let keep4 = class { set [x](_) {} }
let keep5 = class { async [x]() {} }
let keep6 = class { [{ toString() {} }] = 'x' }
