# Diff
## /out.js
### esbuild
```js
// import.js
import { h, render } from "preact";
var p = "p";

// in2.jsx
var Internal = () => /* @__PURE__ */ h(p, null, " Test 2 ");

// app.jsx
var App = () => /* @__PURE__ */ h(p, null, " ", /* @__PURE__ */ h(Internal, null), " T ");
render(/* @__PURE__ */ h(App, null), document.getElementById("app"));
```
### rolldown
```js
import { h as h$1, render as render$1 } from "preact";

//#region import.js
const p = "p";

//#endregion
//#region in2.jsx
const Internal$1 = () => /* @__PURE__ */ h$1(p, null, " Test 2 ");

//#endregion
//#region app.jsx
const App = () => /* @__PURE__ */ h$1(p, null, " ", /* @__PURE__ */ h$1(Internal$1, null), " T ");
render$1(/* @__PURE__ */ h$1(App, null), document.getElementById("app"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	app.js
@@ -1,5 +1,5 @@
-import {h, render} from "preact";
+import {h as h$1, render as render$1} from "preact";
 var p = "p";
-var Internal = () => h(p, null, " Test 2 ");
-var App = () => h(p, null, " ", h(Internal, null), " T ");
-render(h(App, null), document.getElementById("app"));
+var Internal$1 = () => h$1(p, null, " Test 2 ");
+var App = () => h$1(p, null, " ", h$1(Internal$1, null), " T ");
+render$1(h$1(App, null), document.getElementById("app"));

```