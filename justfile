_default:
  just --list -u

init:
  cargo binstall rusty-hook taplo-cli cargo-insta cargo-nextest -y
  yarn install
  git submodule update

test:
  cargo nextest run --no-fail-fast

lint:
  cargo clippy --all -- --deny warnings

# Update our local branch with the remote branch (this is for you to sync the submodules)
update:
  git pull
  git submodule update --init

fmt:
  cargo fmt
  taplo format
  npm run prettier

bench-prepare:
  git clone https://github.com/mrdoob/three.js.git --depth 1 ./temp/three.js  
  for i in {1..10}; do cp -r ./temp/three.js/src ./temp/three.js/copy$i/; done
  
bench:
  cargo bench -p bench

wasm-build:
  cd crates/rolldown_wasm && wasm-pack build
  rm -r ./web/wasm
  mv crates/rolldown_wasm/pkg ./web/wasm
