let a = 0
let b = 0
let obj = { a: { b: 0 } }
let c = 0

a ||= b
obj.a.b ||= c

a &&= b
obj.a.b &&= c
