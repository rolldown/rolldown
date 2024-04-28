// These should be removed because they have no side effects
typeof x_REMOVE
typeof v_REMOVE
typeof f_REMOVE
typeof g_REMOVE
typeof a_REMOVE
var v_REMOVE
function f_REMOVE() {}
function* g_REMOVE() {}
async function a_REMOVE() {}

// These technically have side effects due to TDZ, but this is not currently handled
typeof c_remove
typeof l_remove
typeof s_remove
const c_remove = 0
let l_remove
class s_remove {}