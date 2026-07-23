# Module Types

Rolldown distinguishes the type of content a module contains from the way that content is represented to its importers.

`moduleType` declares the content type, similarly to the HTTP `Content-Type` header. It tells Rolldown and plugins how to interpret the source. For example, Rolldown parses content with `moduleType: 'json'` as JSON even when the module's file extension is not `.json`.

`representType` describes how a module is represented to its importers. It is currently metadata only: setting it does not change parsing, conversion, emission, or bundling behavior.

## Specifying a content type

End users usually do not need to set `moduleType` because Rolldown recognizes known content types from file extensions. For example, it parses and transforms TypeScript and JSX before bundling them as JavaScript.

Sometimes an extension does not identify the content. If a `.data` file contains JSON, use the `moduleTypes` input option to declare that content type:

```js [rolldown.config.js]
export default {
  moduleTypes: {
    '.data': 'json',
  },
};
```

Plugins can declare the content type of a specific module from a `load` or `transform` hook:

```js
const myPlugin = {
  load(id) {
    if (id.endsWith('.data')) {
      return {
        code: '{"answer": 42}',
        moduleType: 'json',
      };
    }
  },
};
```

This shared content-type convention lets plugins cooperate without inferring a type from an id. For example, `@vitejs/plugin-vue` can mark a virtual style block with `moduleType: 'css'`, allowing a CSS plugin to recognize it without understanding Vue-specific query parameters.

Similarly, a JSONC plugin can strip comments in its `load` hook and return the remaining content with `moduleType: 'json'`. Rolldown can then handle the JSON without needing first-class JSONC support.

## Specifying representation metadata

A plugin may also set `representType` in a `load` or `transform` result. Later hooks can read the final explicitly supplied value from `ModuleInfo.representType`:

```js
const metadataPlugin = {
  load(id) {
    if (id.endsWith('.data')) {
      return {
        code: '{"answer": 42}',
        moduleType: 'json',
        representType: 'text',
      };
    }
  },
  moduleParsed(moduleInfo) {
    console.log(moduleInfo.representType); // 'text'
  },
};
```

If multiple hooks return `representType`, the last explicit value wins. If no hook returns it, `ModuleInfo.representType` is `undefined`.

::: warning Metadata only
`representType` does not yet select a loader or change the generated output. Continue to provide valid content and the appropriate `moduleType` for current Rolldown processing.
:::

## Deprecated representation-oriented module types

The following `moduleType` values mix representation choices into the content type API. They remain accepted for compatibility but are deprecated. Use the corresponding `representType` value for representation metadata:

| Deprecated `moduleType` | Use `representType` |
| ----------------------- | ------------------- |
| `base64`                | `base64`            |
| `dataurl`               | `dataurl`           |
| `binary`                | `binary`            |
| `empty`                 | `empty`             |
| `asset`                 | `url`               |
| `copy`                  | `copy`              |
