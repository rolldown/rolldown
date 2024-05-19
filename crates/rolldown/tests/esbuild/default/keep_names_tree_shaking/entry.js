function fnStmtRemove() {}
function fnStmtKeep() {}
x = fnStmtKeep

let fnExprRemove = function remove() {}
let fnExprKeep = function keep() {}
x = fnExprKeep

class clsStmtRemove {}
class clsStmtKeep {}
new clsStmtKeep()

let clsExprRemove = class remove {}
let clsExprKeep = class keep {}
new clsExprKeep()