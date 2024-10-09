function keep(x = foo()) { return x }
console.log(keep(1))
keep(foo())
keep(1)