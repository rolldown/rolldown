# Reason
1. cjs module lexer can't recognize esbuild interop pattern
2. different iife impl
# Diff
## /out.js
### esbuild
```js
var globalName = (() => {
  // entry.js
  var entry_exports = {};
  __export(entry_exports, {
    C: () => Class,
    Class: () => Class,
    Fn: () => Fn,
    abc: () => abc,
    b: () => b_exports,
    c: () => c,
    default: () => entry_default,
    l: () => l,
    v: () => v
  });

  // a.js
  var abc = void 0;

  // b.js
  var b_exports = {};
  __export(b_exports, {
    xyz: () => xyz
  });
  var xyz = null;

  // entry.js
  var entry_default = 123;
  var v = 234;
  var l = 234;
  var c = 234;
  function Fn() {
  }
  var Class = class {
  };
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
var globalName = (function(exports) {

Object.defineProperty(exports, '__esModule', { value: true });
// HIDDEN [rolldown:runtime]

//#region a.js
const abc = void 0;

//#endregion
//#region b.js
var b_exports = {};
__export(b_exports, { xyz: () => xyz });
const xyz = null;

//#endregion
//#region entry.js
var entry_default = 123;
var v = 234;
let l = 234;
const c = 234;
function Fn() {}
var Class = class {};

//#endregion
exports.C = Class;
exports.Class = Class;
exports.Fn = Fn;
exports.abc = abc;
Object.defineProperty(exports, 'b', {
  enumerable: true,
  get: function () {
    return b_exports;
  }
});
exports.c = c;
exports.default = entry_default;
exports.l = l;
exports.v = v;
return exports;
})({});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,27 +1,32 @@
-var globalName = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        C: () => Class,
-        Class: () => Class,
-        Fn: () => Fn,
-        abc: () => abc,
-        b: () => b_exports,
-        c: () => c,
-        default: () => entry_default,
-        l: () => l,
-        v: () => v
+var globalName = (function (exports) {
+    Object.defineProperty(exports, '__esModule', {
+        value: true
     });
-    var abc = void 0;
+    const abc = void 0;
     var b_exports = {};
     __export(b_exports, {
         xyz: () => xyz
     });
-    var xyz = null;
+    const xyz = null;
     var entry_default = 123;
     var v = 234;
-    var l = 234;
-    var c = 234;
+    let l = 234;
+    const c = 234;
     function Fn() {}
     var Class = class {};
-    return __toCommonJS(entry_exports);
-})();
+    exports.C = Class;
+    exports.Class = Class;
+    exports.Fn = Fn;
+    exports.abc = abc;
+    Object.defineProperty(exports, 'b', {
+        enumerable: true,
+        get: function () {
+            return b_exports;
+        }
+    });
+    exports.c = c;
+    exports.default = entry_default;
+    exports.l = l;
+    exports.v = v;
+    return exports;
+})({});

```