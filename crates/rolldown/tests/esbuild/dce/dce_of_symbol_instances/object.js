let remove1 = {}
let remove2 = { *[Symbol.iterator]() {} }
let remove3 = { *[Symbol['iterator']]() {} }

let keep1 = { *[Symbol.iterator]() {}, [keep]: null }
let keep2 = { [keep]: null, *[Symbol.iterator]() {} }
let keep3 = { *[Symbol.wtf]() {} }