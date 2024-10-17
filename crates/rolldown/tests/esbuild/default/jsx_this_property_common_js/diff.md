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

//#region factory.jsx
console.log([this.factory("x", null), /* @__PURE__ */ this.factory("x", null)]);
f = function() {
	console.log([this.factory("y", null), /* @__PURE__ */ this.factory("y", null)]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,9 +1,4 @@
-var require_factory = __commonJS({
-    "factory.jsx"(exports) {
-        console.log([exports.factory("x", null), exports.factory("x", null)]);
-        f = function () {
-            console.log([this.factory("y", null), this.factory("y", null)]);
-        };
-    }
-});
-export default require_factory();
+console.log([this.factory("x", null), this.factory("x", null)]);
+f = function () {
+    console.log([this.factory("y", null), this.factory("y", null)]);
+};

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

//#region fragment.jsx
console.log([this.factory(this.fragment, null, "x"), /* @__PURE__ */ this.factory(this.fragment, null, "x")]), f = function() {
	console.log([this.factory(this.fragment, null, "y"), /* @__PURE__ */ this.factory(this.fragment, null, "y")]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,8 +1,3 @@
-var require_fragment = __commonJS({
-    "fragment.jsx"(exports) {
-        (console.log([exports.factory(exports.fragment, null, "x"), exports.factory(exports.fragment, null, "x")]), f = function () {
-            console.log([this.factory(this.fragment, null, "y"), this.factory(this.fragment, null, "y")]);
-        });
-    }
+(console.log([this.factory(this.fragment, null, "x"), this.factory(this.fragment, null, "x")]), f = function () {
+    console.log([this.factory(this.fragment, null, "y"), this.factory(this.fragment, null, "y")]);
 });
-export default require_fragment();

```