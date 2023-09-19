_default:
  just --list -u

init:
  cargo binstall rusty-hook taplo-cli cargo-insta cargo-nextest -y

test:
  cargo nextest run

lint:
  cargo clippy --all -- --deny warnings

fmt:
  cargo fmt
  taplo format