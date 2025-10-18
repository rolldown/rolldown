# moduleTypes

- **Type:** `Record<string, 'js' | 'jsx' | 'ts' | 'tsx' | 'json' | 'text' | 'base64' | 'dataurl' | 'binary' | 'empty' | 'css' | 'asset'>`
- **Default:** Standard extensions (js, jsx, ts, tsx, json, css, txt) have built-in module types
- **Optional:** Yes ✅

Maps file patterns to module types, controlling how files are processed. This is conceptually similar to esbuild's `loader` option, allowing you to specify how different file extensions should be handled.
