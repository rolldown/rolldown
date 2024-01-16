# Setup Project

## Project Setup

```bash
just init
```

# Project Commands

```bash
just        # Show command list
just init   # Install project tools
just test   # Run project test
just lint   # Run lint
just fmt    # Run format
just bench-prepare && just bench # Run benchmarks

yarn build  # Build node workspace packages and bindings
yarn test   # Run node test
yarn test:update # Update node test snapshot
```

# Adding commands to `justfile` or `package.json`

These ares some guidance suggestions for adding commands to `justfile` or `package.json`,

- `justfile` is intended for control flow and orchestrating commands.
- `justfile` is for workflow related to the rust.
- `package.scripts` is for workflow related to the node.
- Keep npm / cargo commands in `package.scripts` or ``.cargo/config.toml`
