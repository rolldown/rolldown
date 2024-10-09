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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,5 +0,0 @@
-import {h, render} from "preact";
-var p = "p";
-var Internal = () => h(p, null, " Test 2 ");
-var App = () => h(p, null, " ", h(Internal, null), " T ");
-render(h(App, null), document.getElementById("app"));

```