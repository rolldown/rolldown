// Class methods
class Foo0 { fn() {} }
class Foo1 { *fn() {} }
class Foo2 { get fn() {} }
class Foo3 { set fn(_) {} }
class Foo4 { async fn() {} }
class Foo5 { static fn() {} }
class Foo6 { static *fn() {} }
class Foo7 { static get fn() {} }
class Foo8 { static set fn(_) {} }
class Foo9 { static async fn() {} }

// Class private methods
class Bar0 { #fn() {} }
class Bar1 { *#fn() {} }
class Bar2 { get #fn() {} }
class Bar3 { set #fn(_) {} }
class Bar4 { async #fn() {} }
class Bar5 { static #fn() {} }
class Bar6 { static *#fn() {} }
class Bar7 { static get #fn() {} }
class Bar8 { static set #fn(_) {} }
class Bar9 { static async #fn(_) {} }

// Object methods
const Baz0 = { fn() {} }
const Baz1 = { *fn() {} }
const Baz2 = { get fn() {} }
const Baz3 = { set fn(_) {} }
const Baz4 = { async fn() {} }