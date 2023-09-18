_default:
  just --list -u

init:
  cargo binstall cargo-insta cargo-nextest -y

test:
  cargo nextest run

lint:
  cargo clippy --all -- --deny warnings