# Reason
1. rollup `treeshake.annotations` only affect annotations, https://rollupjs.org/repl/?version=4.27.3&shareable=JTdCJTIyZXhhbXBsZSUyMiUzQW51bGwlMkMlMjJtb2R1bGVzJTIyJTNBJTVCJTdCJTIyY29kZSUyMiUzQSUyMmltcG9ydCUyMCU1QyUyMi4lMkZxdXguanMlNUMlMjIlNUNuJTVDbmZ1bmN0aW9uJTIwdGVzdCgpJTIwJTdCJTVDbmNvbnNvbGUubG9nKCd0ZXN0JyklNUNuJTdEJTVDbiU1Q24lMkYqJTIzX19QVVJFX18qJTJGdGVzdCgpJTNCJTIyJTJDJTIyaXNFbnRyeSUyMiUzQXRydWUlMkMlMjJuYW1lJTIyJTNBJTIybWFpbi5qcyUyMiU3RCUyQyU3QiUyMmNvZGUlMjIlM0ElMjJjb25zb2xlLmxvZygndGVzdCcpJTIyJTJDJTIyaXNFbnRyeSUyMiUzQWZhbHNlJTJDJTIybmFtZSUyMiUzQSUyMnF1eC5qcyUyMiU3RCUyQyU3QiUyMmNvZGUlMjIlM0ElMjIlN0IlNUNuJTIwJTIwJTVDJTIyc2lkZUVmZmVjdHMlNUMlMjIlM0ElMjBmYWxzZSU1Q24lN0QlMjIlMkMlMjJpc0VudHJ5JTIyJTNBZmFsc2UlMkMlMjJuYW1lJTIyJTNBJTIycGFja2FnZS5qc29uJTIyJTdEJTVEJTJDJTIyb3B0aW9ucyUyMiUzQSU3QiUyMm91dHB1dCUyMiUzQSU3QiUyMmZvcm1hdCUyMiUzQSUyMmVzJTIyJTdEJTJDJTIydHJlZXNoYWtlJTIyJTNBJTdCJTIyYW5ub3RhdGlvbnMlMjIlM0F0cnVlJTdEJTdEJTdE
# Diff
## /out.js
### esbuild
```js
// keep-me/index.js
console.log("side effects");

// entry.jsx
function KeepMe1() {
}
var keepMe2 = React.createElement(KeepMe1, null);
function keepMe3() {
  console.log("side effects");
}
var keepMe4 = keepMe3();
var keepMe5 = pure();
var keepMe6 = some.fn();
```
### rolldown
```js
//#region entry.jsx
function KeepMe1() {}
let keepMe2 = /* @__PURE__ */ React.createElement(KeepMe1, null);
function keepMe3() {
	console.log("side effects");
}
let keepMe4 = /* @__PURE__ */ keepMe3();
let keepMe5 = pure();
let keepMe6 = some.fn();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,4 @@
-console.log("side effects");
 function KeepMe1() {}
 var keepMe2 = React.createElement(KeepMe1, null);
 function keepMe3() {
     console.log("side effects");

```