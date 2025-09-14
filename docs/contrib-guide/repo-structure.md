# Repo Structure

This document outlines the structure of the repository and the purpose of each directory.

# `/crates`

We store all the Rust crates in this directory.

- `/bench` Benchmark programs for Rust side of the project.
- `/rolldown` Core logic of rolldown the bundler.
- `/rolldown_binding` Glue code that binds the core logic to the Node.js.

# `/packages`

We store all the Node.js packages in this directory.

- `/rolldown` Node.js package for the project.
- `/bench` Benchmark programs for Node.js side of the project.
- `/rollup-tests` Adapter for running rollup tests with rolldown.
- `/vite-tests` Script to run tests in rolldown-vite repo with local rolldown.

# `/examples`

This directory contains examples of how to use `rolldown` in Node.js for various scenarios.

# `/scripts`

This directory contains scripts that are used to automate various tasks for the project.

# `/web`

This directory contains some websites related to the project.

- `/docs` Documentation for the project.
