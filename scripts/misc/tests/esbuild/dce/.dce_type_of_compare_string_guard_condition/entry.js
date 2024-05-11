// Everything here should be removed as dead code due to tree shaking
var REMOVE_1 = typeof x <= 'u' ? x : null
var REMOVE_1 = typeof x < 'u' ? x : null
var REMOVE_1 = typeof x >= 'u' ? null : x
var REMOVE_1 = typeof x > 'u' ? null : x
var REMOVE_1 = typeof x <= 'u' && x
var REMOVE_1 = typeof x < 'u' && x
var REMOVE_1 = typeof x >= 'u' || x
var REMOVE_1 = typeof x > 'u' || x
var REMOVE_1 = 'u' >= typeof x ? x : null
var REMOVE_1 = 'u' > typeof x ? x : null
var REMOVE_1 = 'u' <= typeof x ? null : x
var REMOVE_1 = 'u' < typeof x ? null : x
var REMOVE_1 = 'u' >= typeof x && x
var REMOVE_1 = 'u' > typeof x && x
var REMOVE_1 = 'u' <= typeof x || x
var REMOVE_1 = 'u' < typeof x || x

// Everything here should be kept as live code because it has side effects
var keep_1 = typeof x <= 'u' ? y : null
var keep_1 = typeof x < 'u' ? y : null
var keep_1 = typeof x >= 'u' ? null : y
var keep_1 = typeof x > 'u' ? null : y
var keep_1 = typeof x <= 'u' && y
var keep_1 = typeof x < 'u' && y
var keep_1 = typeof x >= 'u' || y
var keep_1 = typeof x > 'u' || y
var keep_1 = 'u' >= typeof x ? y : null
var keep_1 = 'u' > typeof x ? y : null
var keep_1 = 'u' <= typeof x ? null : y
var keep_1 = 'u' < typeof x ? null : y
var keep_1 = 'u' >= typeof x && y
var keep_1 = 'u' > typeof x && y
var keep_1 = 'u' <= typeof x || y
var keep_1 = 'u' < typeof x || y

// Everything here should be kept as live code because it has side effects
var keep_2 = typeof x <= 'u' ? null : x
var keep_2 = typeof x < 'u' ? null : x
var keep_2 = typeof x >= 'u' ? x : null
var keep_2 = typeof x > 'u' ? x : null
var keep_2 = typeof x <= 'u' || x
var keep_2 = typeof x < 'u' || x
var keep_2 = typeof x >= 'u' && x
var keep_2 = typeof x > 'u' && x
var keep_2 = 'u' >= typeof x ? null : x
var keep_2 = 'u' > typeof x ? null : x
var keep_2 = 'u' <= typeof x ? x : null
var keep_2 = 'u' < typeof x ? x : null
var keep_2 = 'u' >= typeof x || x
var keep_2 = 'u' > typeof x || x
var keep_2 = 'u' <= typeof x && x
var keep_2 = 'u' < typeof x && x