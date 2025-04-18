# Reason
1. inline could be done in minifier
# Diff
## /out.js
### esbuild
```js
// entry.js
console.log(1);
console.log(2);
unknownFn(3);
for (let c = x; ; ) console.log(c);
for (let d in x) console.log(d);
for (let e of x) console.log(e);
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
@@ -1,6 +1,13 @@
-console.log(1);
-console.log(2);
-unknownFn(3);
-for (let c = x; ; ) console.log(c);
-for (let d in x) console.log(d);
-for (let e of x) console.log(e);
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
+for (const c = x; ; ) console.log(c);
+for (const d in x) console.log(d);
+for (const e of x) console.log(e);

```