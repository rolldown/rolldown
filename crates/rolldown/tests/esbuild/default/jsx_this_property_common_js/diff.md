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

```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_factory = __commonJS({
-    "factory.jsx"(exports) {
-        console.log([exports.factory("x", null), exports.factory("x", null)]);
-        f = function () {
-            console.log([this.factory("y", null), this.factory("y", null)]);
-        };
-    }
-});
-export default require_factory();

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

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var require_fragment = __commonJS({
-    "fragment.jsx"(exports) {
-        (console.log([exports.factory(exports.fragment, null, "x"), exports.factory(exports.fragment, null, "x")]), f = function () {
-            console.log([this.factory(this.fragment, null, "y"), this.factory(this.fragment, null, "y")]);
-        });
-    }
-});
-export default require_fragment();

```