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


//#region a.js
const abc = undefined;

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
class Class {}

//#endregion
export { Class as C, Class, Fn, abc, b_exports as b, c, entry_default as default, l, v };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,27 +1,13 @@
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
-    });
-    var abc = void 0;
-    var b_exports = {};
-    __export(b_exports, {
-        xyz: () => xyz
-    });
-    var xyz = null;
-    var entry_default = 123;
-    var v = 234;
-    var l = 234;
-    var c = 234;
-    function Fn() {}
-    var Class = class {};
-    return __toCommonJS(entry_exports);
-})();
+var abc = undefined;
+var b_exports = {};
+__export(b_exports, {
+    xyz: () => xyz
+});
+var xyz = null;
+var entry_default = 123;
+var v = 234;
+var l = 234;
+var c = 234;
+function Fn() {}
+class Class {}
+export {Class as C, Class, Fn, abc, b_exports as b, c, entry_default as default, l, v};

```