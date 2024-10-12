# Diff
## /out.js
### esbuild
```js
// entry.js
var a = 1;
console.log(a);
if (true) {
  const b = 2;
  console.log(b);
}
for (const c = x; ; ) console.log(c);
for (const d in x) console.log(d);
for (const e of x) console.log(e);
```
### rolldown
```js

//#region entry.js
const a = 1;
console.log(a);
{
	const b = 2;
	console.log(b);
}
for (const c = x;;) console.log(c);
for (const d in x) console.log(d);
for (const e of x) console.log(e);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
 var a = 1;
 console.log(a);
-if (true) {
+{
     const b = 2;
     console.log(b);
 }
 for (const c = x; ; ) console.log(c);

```