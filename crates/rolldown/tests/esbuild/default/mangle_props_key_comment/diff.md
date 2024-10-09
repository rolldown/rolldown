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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,5 +0,0 @@
-x("_doNotMangleThis", `_doNotMangleThis`);
-x.a("a", "a");
-x.b("b", "b");
-x.c = ("c" in y);
-x([`foo.${"a"} = bar.${"b"}`, `foo.${"notMangled"} = bar.${"notMangledEither"}`]);

```