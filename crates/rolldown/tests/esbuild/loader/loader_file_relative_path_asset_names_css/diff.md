# Reason
1. css reference .png
2. not support asset path template
# Diff
## /out/entries/entry.css
### esbuild
```js
/* src/entries/entry.css */
div {
  background: url("../images/image-LSAMBFUD.png");
}
```
### rolldown
```js
div {
	background: url(assets/image-6tcw8vpN.png);
}

```
### diff
```diff
===================================================================
--- esbuild	/out/entries/entry.css
+++ rolldown	entries_entry.css
@@ -1,4 +1,3 @@
-/* src/entries/entry.css */
 div {
-  background: url("../images/image-LSAMBFUD.png");
-}
\ No newline at end of file
+	background: url(assets/image-6tcw8vpN.png);
+}

```