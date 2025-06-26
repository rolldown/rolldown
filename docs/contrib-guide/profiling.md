# Profiling

## Memory profiling

To profile memory usage, you can use [`heaptrack`](https://github.com/KDE/heaptrack).

### Setup

First you need to install `heaptrack` and `heaptrack-gui`. If you are using Ubuntu, you can install it with:

```bash
sudo apt install heaptrack heaptrack-gui
```

::: warning

`heaptrack` only supports Linux. It works fine on WSL.

:::

### Build

To build Rolldown with the information required by `heaptrack`, you need to build it with:

```shell
just build-memory-profile
```

### Profiling

After building, you can run Rolldown with the following command to profile memory usage:

```shell
heaptrack node ./path/to/script-rolldown-is-used.js
```

::: tip Using asdf or other version manager that uses shims?

In that case, you may need to use the actual path to the Node binary. For example, if you are using asdf, you can run it with:

```shell
heaptrack $(asdf which node) ./path/to/script-rolldown-is-used.js
```

:::

The heaptrack GUI will open automatically after the script finishes running.

![heaptrack-gui screenshot](./heaptrack-gui.png)
