Rolldown uses the following APIs to watch for changes by default:

- Linux, Android: `inotify`
- macOS: `FSEvents`
- Windows: `ReadDirectoryChangesW`
- BSD descendants (e.g. FreeBSD): `kqueue`
- Other: None (polling)

There are some limitations for each API. If you need to work around them, you can use [`watcher.usePolling`](/reference/Interface.WatcherFileWatcherOptions#usepolling) to force Rolldown to use polling instead of the native API.

::: warning Using on Windows Subsystem for Linux (WSL) 2

When running Rolldown on WSL2, file system watching does not work when a file is edited by Windows applications (non-WSL2 process). This is due to [a WSL2 limitation](https://github.com/microsoft/WSL/issues/4739). This also applies to running on Docker with a WSL2 backend.

To fix it, you could either:

- **Recommended**: Use WSL2 applications to edit your files.
  - It is also recommended to move the project folder outside of a Windows filesystem. Accessing Windows filesystem from WSL2 is slow. Removing that overhead will improve performance.
- Set [`usePolling: true`](/reference/Interface.WatcherFileWatcherOptions#usepolling).
  - Note that `usePolling` leads to higher CPU utilization.

:::
