# Rolldown

Build, test, lint and format commands, as defined in `justfile`:

- Build the `rolldown` package and its native binding: `just build`
- Run the Rust tests: `just test-rust`
- Build, then run the Node.js tests: `just test-node`
- Run all tests: `just test`
- Update Rust and Node.js test snapshots: `just test-update`
- Lint Rust, Node.js and repository files: `just lint`
- Format Rust files with `cargo fmt --all` and all other files with `just fix-repo`
