set shell := ["bash", "-cu"]

_default:
    just --list -u

init:
    cargo binstall rusty-hook taplo-cli cargo-insta -y
    yarn install
    git submodule update

test:
    # TODO: add test for node
    cargo test --no-fail-fast

lint:
    yarn lint-filename
    yarn lint
    # yarn prettier:ci TODO(hyf0): Too slow
    cargo clippy --all -- --deny warnings
    cargo fmt --all -- --check
    taplo format

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

# This command will try to run checks similar to ci locally
check:
  # git diff --exit-code --quiet
  just lint
  just test
  git status
