# Reason
1. could be done in minifier
# Diff
## /out/entry.js
### esbuild
```js
export function outer() {
  {
    let inner = function() {
      return Math.random();
    };
    __name(inner, "inner");
    const x = inner();
    console.log(x);
  }
}
__name(outer, "outer"), outer();
```
### rolldown
```js
//#region entry.js
function outer() {
	{
		function inner() {
			return Math.random();
		}
		const x = inner();
		console.log(x);
	}
}
outer();

//#endregion
export { outer };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,11 +1,11 @@
-export function outer() {
+function outer() {
     {
-        let inner = function () {
+        function inner() {
             return Math.random();
-        };
-        __name(inner, "inner");
+        }
         const x = inner();
         console.log(x);
     }
 }
-(__name(outer, "outer"), outer());
+outer();
+export {outer};

```