# Reason
1. `jsx.factory`
# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
var require_factory = __commonJS({
  "factory.jsx"(exports) {
    console.log([
      /* @__PURE__ */ exports.factory("x", null),
      /* @__PURE__ */ exports.factory("x", null)
    ]);
    f = function() {
      console.log([
        /* @__PURE__ */ this.factory("y", null),
        /* @__PURE__ */ this.factory("y", null)
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
	console.log([this.factory("x", null), /* @__PURE__ */ exports.factory("x", null)]);
	f = function() {
		console.log([this.factory("y", null), /* @__PURE__ */ this.factory("y", null)]);
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
@@ -1,7 +1,8 @@
+import {__commonJS} from "./chunk.js";
 var require_factory = __commonJS({
     "factory.jsx"(exports) {
-        console.log([exports.factory("x", null), exports.factory("x", null)]);
+        console.log([this.factory("x", null), exports.factory("x", null)]);
         f = function () {
             console.log([this.factory("y", null), this.factory("y", null)]);
         };
     }

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
var require_fragment = __commonJS({
  "fragment.jsx"(exports) {
    console.log([
      /* @__PURE__ */ exports.factory(exports.fragment, null, "x"),
      /* @__PURE__ */ exports.factory(exports.fragment, null, "x")
    ]), f = function() {
      console.log([
        /* @__PURE__ */ this.factory(this.fragment, null, "y"),
        /* @__PURE__ */ this.factory(this.fragment, null, "y")
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
	console.log([this.factory(this.fragment, null, "x"), /* @__PURE__ */ exports.factory(exports.fragment, null, "x")]), f = function() {
		console.log([this.factory(this.fragment, null, "y"), /* @__PURE__ */ this.factory(this.fragment, null, "y")]);
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
@@ -1,7 +1,8 @@
+import {__commonJS} from "./chunk.js";
 var require_fragment = __commonJS({
     "fragment.jsx"(exports) {
-        (console.log([exports.factory(exports.fragment, null, "x"), exports.factory(exports.fragment, null, "x")]), f = function () {
+        (console.log([this.factory(this.fragment, null, "x"), exports.factory(exports.fragment, null, "x")]), f = function () {
             console.log([this.factory(this.fragment, null, "y"), this.factory(this.fragment, null, "y")]);
         });
     }
 });

```