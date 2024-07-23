import './remove-me'
function RemoveMe1() {}
let removeMe2 = 0
class RemoveMe3 {}

import './keep-me'
function KeepMe1() {}
let keepMe2 = <KeepMe1/>
function keepMe3() { console.log('side effects') }
let keepMe4 = /* @__PURE__ */ keepMe3()
let keepMe5 = pure()
let keepMe6 = some.fn()