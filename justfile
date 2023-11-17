set shell := ["bash", "-cu"]

_default:
    just --list -u

init:
    cargo binstall rusty-hook taplo-cli cargo-insta wasm-pack -y
    yarn install
    git submodule update

test:
    cargo test --no-fail-fast

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


setup-bench:
    git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./temp/three
    echo "import * as three from './src/Three.js'; export { three }" > temp/three/entry.js
    mkdir -p temp/three10x
    for i in {1..10}; do cp -r ./temp/three/src ./temp/three10x/copy$i/; done
    echo > temp/three10x/entry.js
    for i in {1..10}; do echo "import * as three$i from './copy$i/Three.js'; export { three$i }" >> temp/three10x/entry.js; done

bench:
    cargo bench -p bench

# build wasm of rolldown and move the output `pkg/` under `web` directory
