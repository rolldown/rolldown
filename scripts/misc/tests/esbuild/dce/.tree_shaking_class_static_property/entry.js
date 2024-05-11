let remove1 = class { static x }
let remove3 = class { static x() {} }
let remove4 = class { static get x() {} }
let remove5 = class { static set x(_) {} }
let remove6 = class { static async x() {} }
let remove8 = class { static ['x']() {} }
let remove9 = class { static get ['x']() {} }
let remove10 = class { static set ['x'](_) {} }
let remove11 = class { static async ['x']() {} }
let remove12 = class { static [0] = 'x' }
let remove13 = class { static [null] = 'x' }
let remove14 = class { static [undefined] = 'x' }
let remove15 = class { static [false] = 'x' }
let remove16 = class { static [0n] = 'x' }
let remove17 = class { static toString() {} }

let keep1 = class { static x = x }
let keep2 = class { static ['x'] = x }
let keep3 = class { static [x] = 'x' }
let keep4 = class { static [x]() {} }
let keep5 = class { static get [x]() {} }
let keep6 = class { static set [x](_) {} }
let keep7 = class { static async [x]() {} }
let keep8 = class { static [{ toString() {} }] = 'x' }