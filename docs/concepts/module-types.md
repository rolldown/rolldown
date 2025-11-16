# Module Types

As a web bundler, JavaScript is not the only file type with built-in support in Rolldown. For example, Rolldown can handle TypeScript and JSX files directly, parsing and transforming them to JavaScript before bundling them. We refer to these file types with first-class support in Rolldown as **Module Types**.

## How module types affect users

End users usually do not need to concern themselves with Module Types, since Rolldown automatically recognizes and handles known Module Types.

By default, Rolldown determines the module type of a module based on its file extension. However, in some cases this may not be sufficient. For example, imagine a file containing JSON data, but its extension is `.data`. Rolldown can't recognize it as a JSON file because the extension is not `.json`.

In this case, users need to explicitly tell Rolldown that files with the `.data` extension should be treated as the JSON module type. This can be done via the `moduleTypes` option in the config:

```js [rolldown.config.js]
export default {
  moduleTypes: {
    '.data': 'json',
  },
};
```

## Module types and plugins

Plugins can specify the module type of a specific file via the `load` hook and the `transform` hook:

```js
const myPlugin = {
  load(id) {
    if (id.endsWith('.data')) {
      return {
        code: '...',
        moduleType: 'json',
      };
    }
  },
};
```

The main significance of module types is that it provides a central convention for supported types, making it easier to chain multiple plugins that need to operate on the same module type.

For example, `@vitejs/plugin-vue` currently creates virtual css modules for the style blocks in `.vue` files and append `?lang=css` to the id of a virtual module, allowing these modules to be recognized as css by the vue plugin. However, this is only a convention of the vue plugin - other plugins may ignore the query string and thus not recognize the convention.

With module types, `@vitejs/plugin-vue` can explicitly specify the module type of virtual css modules as `css`, and other plugins like the postcss plugin can process these css modules without being aware of the vue plugin.

Another example: to add support for `.jsonc` files, a plugin could simply strip comments of `.jsonc` files in the `load` hook and return `moduleType: 'json'`. Rolldown will handle the rest.
