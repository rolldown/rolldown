# Module Type

As a web bundler, rolldown should not only accept JavaScript files as input items, but also support other types of input items, such as CSS(in the future), JSON, dataurl, and so on.

Therefore, if an input item could be recognized by rolldown without any extra plugins, it means that the type of this input item is treated as a first-class citizen by rolldown.

We call these fist-class types, `Module Type`s.

## How it affects users

In most scenarios, users don't need to care about this concept, because rolldown will automatically handle it for users.

Rolldown uses the extension of the file to determine what the `Module Type` is, but sometimes it's not enough. For example, a file contains JSON data, but the extension is `.data`. In this case, rolldown can't recognize it as a JSON file, because the extension is not `.json`.

In this case, users need to tell rolldown what the `Module Type` is for the `.data` extension by configuring the `moduleTypes` field in `rolldown.config.mjs`.

```js [rolldown.config.mjs]
export default {
  moduleTypes: {
    '.data': 'json',
  },
}
```

### For plugins

Plugin authors could also specify the `Module Type` for files in many places, such as the `load` hook, and the `transform` hook.

`Module Type` creates official conventions for plugins to follow, so that independent plugins can process certain types of files in a consistent way and have no need to care about the details of other plugins.

For example, vite will create a virtual css module for `.vue` file and append `?lang=css` to id of virtual module, which makes these modules recognized as css modules by the vue plugin. But this is only a convention of the vue plugin, and other plugins may not follow this convention.

Now with `Module Type`, vite can specify the `Module Type` of virtual css modules as `css`, and other plugins like the postcss plugin can process these css modules without knowing details of vue plugin.

Another feature of `Module Type` is that it makes support for new types of files by plugins easier. For example, to add support for `.jsonc` files, a plugin could simply strip comments of `.jsonc` files in the `load` hook and specify the `Module Type` as `json`. Rolldown will handle the rest.
