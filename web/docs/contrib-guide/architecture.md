<script setup>
import Mermaid from '../.vitepress/components/Mermaid.vue'
</script>

# Architecture

This document covers the organization of the repository, as well as some high level discussion of how Rolldown works. The goal of this document is to provide an entry point for understanding the project layout. The primary audience is people who might be interested in modifying or contributing to the Rolldown source code.

## Source code layout

A detailed description of the repository layout is provided [in the contribution guide](https://rolldown.rs/contrib-guide/repo-structure). Below is an overview of the sections which are most important for this document.

- `crates`: Contains the source code for the Rust libraries which power the [`rolldown` npm package](http://npmjs.com/package/rolldown).
- `packages`: Most importantly, contains the source code for the [`rolldown` npm package](http://npmjs.com/package/rolldown). Also contains a set of benchmarks, and a set of tests for Rollup.
- `rollup`: A submodule of the [Rollup library](https://github.com/rollup/rollup/tree/061a0387c8654222620f602471d66afd3c582048) used to ensure compability while developing Rolldown.

The root of the repository contains a number of configuration files for both Rust and Node.js toolchains, as well as additional directories for managing the project. As this document is focused on providing an introduction to the source code for the Rolldown library, the source code in `crates` and `packages` will be the most relevant sections.

## Bird's eye view

Rolldown is primarily two things:

1. A bundler written in Rust
2. A CLI and Node.js package published to npm, which delegates bundling functionality to the Rust crate

The bundler is designed to be fully compatible with [Rollup](https://rollupjs.org).

<!-- TODO: more detail about what the bundler does -->

## The CLI

The Rolldown CLI is a thin wrapper around the [Node.js bindings](https://github.com/rolldown/rolldown/tree/2011bf463b8cead1903375046643abb1168ef46f/crates/rolldown_binding) of the [`rolldown` Rust crate](https://github.com/rolldown/rolldown/tree/2011bf463b8cead1903375046643abb1168ef46f/crates/rolldown). Here's a simple flowchart describing the process of calling the `rolldown` CLI command.

```mermaid
---
title: Bundling code with Rolldown npm package
---
flowchart TB
    subgraph CLI
        invokeCli(["Invoke `rolldown` CLI command"]) --> parse([Parse arguments])
        parse --> loadConfig([Load configuration file])
        loadConfig --> configExists{
            Configuration
            file exists?
        }
        configExists -->|no|exitWithError([Exit with error])

        printStats([
            Collect, transform,
            and print bundle statistics.
        ])
    end
    
    configExists-->|yes|instantiateBuild
    
    subgraph Bundle
        instantiateBuild([
            Instantiate `RolldownBuild`
            class with config options
        ])
        loadNativeBinding([
            Load appropriate native
            binding for system
        ])
        writeOutput([
            Delegate to Rust for bundling
            and writing output to disk
        ])

        instantiateBuild --> loadNativeBinding
        loadNativeBinding --> writeOutput
    end

    writeOutput --> printStats
```

## The Bundler

The Rolldown bundler is the 

```js
const thing = [1, 2, 3]
function main() {
    console.log(thing)
}
```


<!-- 


    subgraph Rust
        initializeRolldownBundler([
            Rolldown binding initializes a Rolldown Bundler
            and calls the write function
        ])
        %% TODO
        %% Basically what happens is it instantiates a Bundler instance from the rolldown_bindings lib
        %% crates/rolldown_binding/src/bundler.rs, which depends on the rolldown crate (crates/rolldown/src/bundler.rs)
        %% there is a little pomp and circumstance, but it basically
        %% just invokes the write method from the main Rolldown Bundler crates/rolldown/src/bundler.rs
        %% This feels a little too in-the-weeds for this flowchart
    end
    
 -->

- RolldownBuild is the primary class (packages/rolldown/src/rolldown-build.ts); the `rolldown` function exported from rolldown.ts simply initializes a RolldownBuild class and returns the instance
    - the `rolldown` function is the primary function used in the `bundle` cli command
    - the `write` method is the primary public interface. It does the following
        - gets or initializes the bundler, which is the Node binding for the Rust Bundler struct
        - calls bundler.write()
        - returns a detailed structure of the transformed Rolldown output
- `defineConfig` is simply a wrapper for correct typing; it just returns the config object that is passed to it
- cli/index.ts exports the `rolldown` command
- running the `rolldown` cli command follows a pretty simple process
    - parse arguments
    - load config files
    - execute `bundle` (packages/rolldown/src/cli/commands/bundle.ts), which does the following (roughly)
        - instantiates a RolldownBuild class
        - calls `.write`
        - prints information about the build to the console

<!-- 
Inspiration:
- https://github.com/redis/redis/blob/f4481e657f905074fa515701af3f695757817d88/README.md#source-code-layout
- https://github.com/rust-lang/rust-analyzer/blob/d9c29afaee6cb26044b5a605e0073fcabb2e9722/docs/dev/architecture.md
- https://github.com/evanw/esbuild/blob/44e746965d783646f97daf3d0617ff816727e7fb/docs/architecture.md
 -->
