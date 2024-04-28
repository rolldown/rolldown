// Everything here should be removed as dead code due to tree shaking
var REMOVE_1 = typeof x !== 'undefined' ? x : null
var REMOVE_1 = typeof x != 'undefined' ? x : null
var REMOVE_1 = typeof x === 'undefined' ? null : x
var REMOVE_1 = typeof x == 'undefined' ? null : x
var REMOVE_1 = typeof x !== 'undefined' && x
var REMOVE_1 = typeof x != 'undefined' && x
var REMOVE_1 = typeof x === 'undefined' || x
var REMOVE_1 = typeof x == 'undefined' || x
var REMOVE_1 = 'undefined' !== typeof x ? x : null
var REMOVE_1 = 'undefined' != typeof x ? x : null
var REMOVE_1 = 'undefined' === typeof x ? null : x
var REMOVE_1 = 'undefined' == typeof x ? null : x
var REMOVE_1 = 'undefined' !== typeof x && x
var REMOVE_1 = 'undefined' != typeof x && x
var REMOVE_1 = 'undefined' === typeof x || x
var REMOVE_1 = 'undefined' == typeof x || x

// Everything here should be removed as dead code due to tree shaking
var REMOVE_2 = typeof x === 'object' ? x : null
var REMOVE_2 = typeof x == 'object' ? x : null
var REMOVE_2 = typeof x !== 'object' ? null : x
var REMOVE_2 = typeof x != 'object' ? null : x
var REMOVE_2 = typeof x === 'object' && x
var REMOVE_2 = typeof x == 'object' && x
var REMOVE_2 = typeof x !== 'object' || x
var REMOVE_2 = typeof x != 'object' || x
var REMOVE_2 = 'object' === typeof x ? x : null
var REMOVE_2 = 'object' == typeof x ? x : null
var REMOVE_2 = 'object' !== typeof x ? null : x
var REMOVE_2 = 'object' != typeof x ? null : x
var REMOVE_2 = 'object' === typeof x && x
var REMOVE_2 = 'object' == typeof x && x
var REMOVE_2 = 'object' !== typeof x || x
var REMOVE_2 = 'object' != typeof x || x

// Everything here should be kept as live code because it has side effects
var keep_1 = typeof x !== 'object' ? x : null
var keep_1 = typeof x != 'object' ? x : null
var keep_1 = typeof x === 'object' ? null : x
var keep_1 = typeof x == 'object' ? null : x
var keep_1 = typeof x !== 'object' && x
var keep_1 = typeof x != 'object' && x
var keep_1 = typeof x === 'object' || x
var keep_1 = typeof x == 'object' || x
var keep_1 = 'object' !== typeof x ? x : null
var keep_1 = 'object' != typeof x ? x : null
var keep_1 = 'object' === typeof x ? null : x
var keep_1 = 'object' == typeof x ? null : x
var keep_1 = 'object' !== typeof x && x
var keep_1 = 'object' != typeof x && x
var keep_1 = 'object' === typeof x || x
var keep_1 = 'object' == typeof x || x

// Everything here should be kept as live code because it has side effects
var keep_2 = typeof x !== 'undefined' ? y : null
var keep_2 = typeof x != 'undefined' ? y : null
var keep_2 = typeof x === 'undefined' ? null : y
var keep_2 = typeof x == 'undefined' ? null : y
var keep_2 = typeof x !== 'undefined' && y
var keep_2 = typeof x != 'undefined' && y
var keep_2 = typeof x === 'undefined' || y
var keep_2 = typeof x == 'undefined' || y
var keep_2 = 'undefined' !== typeof x ? y : null
var keep_2 = 'undefined' != typeof x ? y : null
var keep_2 = 'undefined' === typeof x ? null : y
var keep_2 = 'undefined' == typeof x ? null : y
var keep_2 = 'undefined' !== typeof x && y
var keep_2 = 'undefined' != typeof x && y
var keep_2 = 'undefined' === typeof x || y
var keep_2 = 'undefined' == typeof x || y

// Everything here should be kept as live code because it has side effects
var keep_3 = typeof x !== 'undefined' ? null : x
var keep_3 = typeof x != 'undefined' ? null : x
var keep_3 = typeof x === 'undefined' ? x : null
var keep_3 = typeof x == 'undefined' ? x : null
var keep_3 = typeof x !== 'undefined' || x
var keep_3 = typeof x != 'undefined' || x
var keep_3 = typeof x === 'undefined' && x
var keep_3 = typeof x == 'undefined' && x
var keep_3 = 'undefined' !== typeof x ? null : x
var keep_3 = 'undefined' != typeof x ? null : x
var keep_3 = 'undefined' === typeof x ? x : null
var keep_3 = 'undefined' == typeof x ? x : null
var keep_3 = 'undefined' !== typeof x || x
var keep_3 = 'undefined' != typeof x || x
var keep_3 = 'undefined' === typeof x && x
var keep_3 = 'undefined' == typeof x && x