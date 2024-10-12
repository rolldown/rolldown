# Diff
## /out.js
### esbuild
```js
// keep-me/index.js
console.log("side effects");

// entry.jsx
function KeepMe1() {
}
var keepMe2 = React.createElement(KeepMe1, null);
function keepMe3() {
  console.log("side effects");
}
var keepMe4 = keepMe3();
var keepMe5 = pure();
var keepMe6 = some.fn();
```
### rolldown
```js
import { jsx as _jsx } from "react/jsx-runtime";

//#region remove-me.js
var remove_me_default = "unused";

//#endregion
//#region keep-me/index.js
console.log("side effects");

//#endregion
//#region entry.jsx
function RemoveMe1() {}
let removeMe2 = 0;
class RemoveMe3 {}
function KeepMe1() {}
let keepMe2 = _jsx(KeepMe1, {});
function keepMe3() {
	console.log("side effects");
}
let keepMe4 = /* @__PURE__ */ keepMe3();
let keepMe5 = pure();
let keepMe6 = some.fn();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,12 @@
+import {jsx as _jsx} from "react/jsx-runtime";
+var remove_me_default = "unused";
 console.log("side effects");
+function RemoveMe1() {}
+var removeMe2 = 0;
+class RemoveMe3 {}
 function KeepMe1() {}
-var keepMe2 = React.createElement(KeepMe1, null);
+var keepMe2 = _jsx(KeepMe1, {});
 function keepMe3() {
     console.log("side effects");
 }
 var keepMe4 = keepMe3();

```