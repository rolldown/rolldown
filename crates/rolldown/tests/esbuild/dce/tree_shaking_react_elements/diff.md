# Diff
## /out.js
### esbuild
```js
// entry.jsx
function Foo() {
}
var d = /* @__PURE__ */ React.createElement("div", null);
var e = /* @__PURE__ */ React.createElement(Foo, null, d);
var f = /* @__PURE__ */ React.createElement(React.Fragment, null, e);
console.log(f);
```
### rolldown
```js
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region entry.jsx
function Foo() {}
let a = _jsx("div", {});
let b = _jsx(Foo, { children: a });
let c = _jsx(_Fragment, { children: b });
let d = _jsx("div", {});
let e = _jsx(Foo, { children: d });
let f = _jsx(_Fragment, { children: e });
console.log(f);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,17 @@
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
 function Foo() {}
-var d = React.createElement("div", null);
-var e = React.createElement(Foo, null, d);
-var f = React.createElement(React.Fragment, null, e);
+var a = _jsx("div", {});
+var b = _jsx(Foo, {
+    children: a
+});
+var c = _jsx(_Fragment, {
+    children: b
+});
+var d = _jsx("div", {});
+var e = _jsx(Foo, {
+    children: d
+});
+var f = _jsx(_Fragment, {
+    children: e
+});
 console.log(f);

```