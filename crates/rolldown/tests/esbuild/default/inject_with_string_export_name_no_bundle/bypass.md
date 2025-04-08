# Reason
1. replace the function it self in `inject files`, this align with `@rollup/plugin-inject`
# Diff
## /out.js
### esbuild
```js
var old = console.log;
var fn = (...args) => old.apply(console, ["log:"].concat(args));
fn(test);
```
### rolldown
```js

//#region inject.js
const old = $inject_console_log;
const $inject_console_log = (...args) => old.apply(console, ["log:"].concat(args));

//#endregion
//#region entry.js
$inject_console_log(test);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-var old = console.log;
-var fn = (...args) => old.apply(console, ["log:"].concat(args));
-fn(test);
+var old = $inject_console_log;
+var $inject_console_log = (...args) => old.apply(console, ["log:"].concat(args));
+$inject_console_log(test);

```