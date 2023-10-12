# Setup Project

## Install Rust

```bash
# https://rustup.rs/
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Install binstall

```bash
# https://github.com/cargo-bins/cargo-binstall
cargo install cargo-binstall
```

## Install just

```bash
# https://github.com/casey/just
cargo binstall just -y
```

# Project Commands

```bash
just        # Show command list
just init   # Install project tools
just test   # Run project test
just lint   # Run lint
just fmt    # Run format
just bench-prepare && just bench # Run benchmarks
```
