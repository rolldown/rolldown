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

bench-prepare:
  git clone https://github.com/mrdoob/three.js.git --depth 1 ./temp/three.js  
  for i in {1..10}; do cp -r ./temp/three.js/src ./temp/three.js/copy$i/; done
  
bench:
  cargo bench -p bench