class A { static foo }
class B { static name }
class C { static name() {} }
class D { static get name() {} }
class E { static set name(x) {} }
class F { static ['name'] = 0 }

let a = class a { static foo }
let b = class b { static name }
let c = class c { static name() {} }
let d = class d { static get name() {} }
let e = class e { static set name(x) {} }
let f = class f { static ['name'] = 0 }

let a2 = class { static foo }
let b2 = class { static name }
let c2 = class { static name() {} }
let d2 = class { static get name() {} }
let e2 = class { static set name(x) {} }
let f2 = class { static ['name'] = 0 }