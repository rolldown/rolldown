# Diff
## /out.js
### esbuild
```js
const a = 1;
console.log(1), console.log(2), unknownFn(3);
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
{
	const b = 3;
	unknownFn(b);
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
@@ -1,5 +1,13 @@
-const a = 1;
-(console.log(1), console.log(2), unknownFn(3));
+var a = 1;
+console.log(a);
+{
+    const b = 2;
+    console.log(b);
+}
+{
+    const b = 3;
+    unknownFn(b);
+}
 for (const c = x; ; ) console.log(c);
 for (const d in x) console.log(d);
 for (const e of x) console.log(e);

```