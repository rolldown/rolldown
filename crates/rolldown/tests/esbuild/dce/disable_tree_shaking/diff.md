# Reason
1. rolldown don't have `ignoreDCEAnnotations` option
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

//#region entry.jsx
function KeepMe1() {}
let keepMe2 = _jsx(KeepMe1, {});
let keepMe5 = pure();
let keepMe6 = some.fn();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +1,5 @@
-console.log("side effects");
+import {jsx as _jsx} from "react/jsx-runtime";
 function KeepMe1() {}
-var keepMe2 = React.createElement(KeepMe1, null);
-function keepMe3() {
-    console.log("side effects");
-}
-var keepMe4 = keepMe3();
+var keepMe2 = _jsx(KeepMe1, {});
 var keepMe5 = pure();
 var keepMe6 = some.fn();

```