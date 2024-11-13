# Reason
1. not support public path
# Diff
## /out/entries/entry.css
### esbuild
```js
/* src/entries/entry.css */
div {
  background: url("https://example.com/image-LSAMBFUD.png");
}
```
### rolldown
```js
div {
	background: url(assets/image-Dq1zDy-k.png);
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
-  background: url("https://example.com/image-LSAMBFUD.png");
-}
\ No newline at end of file
+	background: url(assets/image-Dq1zDy-k.png);
+}

```