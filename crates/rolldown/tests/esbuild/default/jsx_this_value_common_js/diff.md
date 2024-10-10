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

```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_factory = __commonJS({
-    "factory.jsx"(exports) {
-        console.log([exports("x", null), exports("x", null)]);
-        f = function () {
-            console.log([this("y", null), this("y", null)]);
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

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var require_fragment = __commonJS({
-    "fragment.jsx"(exports) {
-        (console.log([exports(exports, null, "x"), exports(exports, null, "x")]), f = function () {
-            console.log([this(this, null, "y"), this(this, null, "y")]);
-        });
-    }
-});
-export default require_fragment();

```