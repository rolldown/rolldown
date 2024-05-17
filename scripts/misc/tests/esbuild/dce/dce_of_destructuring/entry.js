// Identifier bindings
var remove1
var remove2 = null
var KEEP1 = x

// Array patterns
var [remove3] = []
var [remove4, ...remove5] = [...[1, 2], 3]
var [, , remove6] = [, , 3]
var [KEEP2] = [x]
var [KEEP3] = [...{}]

// Object patterns (not handled right now)
var { KEEP4 } = {}