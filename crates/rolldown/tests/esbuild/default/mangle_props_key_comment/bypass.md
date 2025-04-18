# Reason
1. could be done in minifier
# Diff
## /out/entry.js
### esbuild
```js
x(
  /* __KEY__ */
  "_doNotMangleThis",
  /* __KEY__ */
  `_doNotMangleThis`
);
x.a(/* @__KEY__ */ "a", /* @__KEY__ */ "a");
x.b(/* @__KEY__ */ "b", /* @__KEY__ */ "b");
x.c = /* @__KEY__ */ "c" in y;
x([
  `foo.${/* @__KEY__ */ "a"} = bar.${/* @__KEY__ */ "b"}`,
  `foo.${/* @__KEY__ */ "notMangled"} = bar.${/* @__KEY__ */ "notMangledEither"}`
]);
```
### rolldown
```js
//#region entry.js
x(
	/* __KEY__ */
	"_doNotMangleThis",
	/* __KEY__ */
	`_doNotMangleThis`
);
x._mangleThis(
	/* @__KEY__ */
	"_mangleThis",
	/* @__KEY__ */
	`_mangleThis`
);
x._mangleThisToo(
	/* #__KEY__ */
	"_mangleThisToo",
	/* #__KEY__ */
	`_mangleThisToo`
);
x._someKey = "_someKey" in y;
x([`foo._mangleThis = bar._mangleThisToo`, `foo.notMangled = bar.notMangledEither`]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
 x("_doNotMangleThis", `_doNotMangleThis`);
-x.a("a", "a");
-x.b("b", "b");
-x.c = ("c" in y);
-x([`foo.${"a"} = bar.${"b"}`, `foo.${"notMangled"} = bar.${"notMangledEither"}`]);
+x._mangleThis("_mangleThis", `_mangleThis`);
+x._mangleThisToo("_mangleThisToo", `_mangleThisToo`);
+x._someKey = ("_someKey" in y);
+x([`foo._mangleThis = bar._mangleThisToo`, `foo.notMangled = bar.notMangledEither`]);

```