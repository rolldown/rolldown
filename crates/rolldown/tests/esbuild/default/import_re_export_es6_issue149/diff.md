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
import { h, render } from "preact";

//#region import.js
const Part = "p";

//#endregion
//#region in2.jsx
const Internal = () => /* @__PURE__ */ h(Part, null, " Test 2 ");

//#endregion
//#region app.jsx
const App = () => /* @__PURE__ */ h(Part, null, " ", /* @__PURE__ */ h(Internal, null), " T ");
render(/* @__PURE__ */ h(App, null), document.getElementById("app"));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	app.js
@@ -1,5 +1,5 @@
 import {h, render} from "preact";
-var p = "p";
-var Internal = () => h(p, null, " Test 2 ");
-var App = () => h(p, null, " ", h(Internal, null), " T ");
+var Part = "p";
+var Internal = () => h(Part, null, " Test 2 ");
+var App = () => h(Part, null, " ", h(Internal, null), " T ");
 render(h(App, null), document.getElementById("app"));

```