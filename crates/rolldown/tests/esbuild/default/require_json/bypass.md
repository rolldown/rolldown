# Reason
1. skip quote
# Diff
## /out.js
### esbuild
```js
// test.json
var require_test = __commonJS({
  "test.json"(exports, module) {
    module.exports = {
      a: true,
      b: 123,
      c: [null]
    };
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js



//#region test.json
var require_test = __commonJS({ "test.json"(exports, module) {
	module.exports = {
		"a": true,
		"b": 123,
		"c": [null]
	};
} });
//#endregion

//#region entry.js
console.log(require_test());
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,10 @@
 var require_test = __commonJS({
     "test.json"(exports, module) {
         module.exports = {
-            a: true,
-            b: 123,
-            c: [null]
+            "a": true,
+            "b": 123,
+            "c": [null]
         };
     }
 });
 console.log(require_test());

```