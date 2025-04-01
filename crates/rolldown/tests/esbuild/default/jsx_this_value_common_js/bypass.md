# Reason
1. rolldown auto code splitting
# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
var require_factory = __commonJS({
  "factory.jsx"(exports) {
    console.log([
      /* @__PURE__ */ exports("x", null),
      /* @__PURE__ */ exports("x", null)
    ]);
    f = function() {
      console.log([
        /* @__PURE__ */ this("y", null),
        /* @__PURE__ */ this("y", null)
      ]);
    };
  }
});
export default require_factory();
```
### rolldown
```js
import { __commonJS } from "./chunk.js";

//#region factory.jsx
var require_factory = __commonJS({ "factory.jsx"(exports) {
	console.log([/* @__PURE__ */ exports("x", null), /* @__PURE__ */ exports("x", null)]);
	f = function() {
		console.log([/* @__PURE__ */ this("y", null), /* @__PURE__ */ this("y", null)]);
	};
} });
//#endregion

export default require_factory();

```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,4 +1,5 @@
+import {__commonJS} from "./chunk.js";
 var require_factory = __commonJS({
     "factory.jsx"(exports) {
         console.log([exports("x", null), exports("x", null)]);
         f = function () {

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
var require_fragment = __commonJS({
  "fragment.jsx"(exports) {
    console.log([
      /* @__PURE__ */ exports(exports, null, "x"),
      /* @__PURE__ */ exports(exports, null, "x")
    ]), f = function() {
      console.log([
        /* @__PURE__ */ this(this, null, "y"),
        /* @__PURE__ */ this(this, null, "y")
      ]);
    };
  }
});
export default require_fragment();
```
### rolldown
```js
import { __commonJS } from "./chunk.js";

//#region fragment.jsx
var require_fragment = __commonJS({ "fragment.jsx"(exports) {
	console.log([/* @__PURE__ */ exports(exports, null, "x"), /* @__PURE__ */ exports(exports, null, "x")]), f = function() {
		console.log([/* @__PURE__ */ this(this, null, "y"), /* @__PURE__ */ this(this, null, "y")]);
	};
} });
//#endregion

export default require_fragment();

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,4 +1,5 @@
+import {__commonJS} from "./chunk.js";
 var require_fragment = __commonJS({
     "fragment.jsx"(exports) {
         (console.log([exports(exports, null, "x"), exports(exports, null, "x")]), f = function () {
             console.log([this(this, null, "y"), this(this, null, "y")]);

```