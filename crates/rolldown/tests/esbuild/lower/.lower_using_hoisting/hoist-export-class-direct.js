using a = b
export class Foo1 { ac = [a, c] }
export class Bar1 { ac = [a, c, Bar1] }
class Foo2 { ac = [a, c] }
class Bar2 { ac = [a, c, Bar2] }
using c = d