list:
  just --list

format:
  cargo fmt --all

build:
  cargo build --all --all-features

build-wasm:
  cargo build --manifest-path ./platform/data/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./platform/derive/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./platform/core/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./platform/parser/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./platform/nodes/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./backends/vm/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frameworks/dynamic/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frameworks/pointer/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frameworks/value/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frameworks/serde/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frameworks/text/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frameworks/arena/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frontends/serde/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frontends/assembler/Cargo.toml --all-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frontends/simpleton/Cargo.toml --no-default-features --target wasm32-unknown-unknown
  cargo build --manifest-path ./frontends/vault/Cargo.toml --all-features --target wasm32-unknown-unknown

test:
  cargo test --all --all-features
  cd ./runners/simpleton && just run
  cd ./runners/alchemyst && just run

miri:
  cargo +nightly miri test --manifest-path ./platform/data/Cargo.toml
  cargo +nightly miri test --manifest-path ./platform/core/Cargo.toml

bench NAME="all":
  cargo run --manifest-path ./benches/Cargo.toml --no-default-features --features=bench_{{NAME}} --release
  # cargo build --manifest-path ./benches/Cargo.toml --no-default-features --features=bench_{{NAME}}

clippy:
  cargo clippy --all --all-features
  cargo clippy --tests --all --all-features

checks:
  just format
  just build
  just build-wasm
  just clippy
  just test
  just miri

clean:
  find . -name target -type d -exec rm -r {} +
  just remove-lockfiles

remove-lockfiles:
  find . -name Cargo.lock -type f -exec rm {} +

list-outdated:
  cargo outdated -R -w

update:
  cargo update --manifest-path ./platform/data/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/derive/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/core/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/parser/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/plugins/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/nodes/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/ffi/Cargo.toml --aggressive
  cargo update --manifest-path ./backends/vm/Cargo.toml --aggressive
  cargo update --manifest-path ./frameworks/dynamic/Cargo.toml --aggressive
  cargo update --manifest-path ./frameworks/pointer/Cargo.toml --aggressive
  cargo update --manifest-path ./frameworks/value/Cargo.toml --aggressive
  cargo update --manifest-path ./frameworks/serde/Cargo.toml --aggressive
  cargo update --manifest-path ./frameworks/text/Cargo.toml --aggressive
  cargo update --manifest-path ./frameworks/arena/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/serde/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/assembler/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/simpleton/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/vault/Cargo.toml --aggressive
  cargo update --manifest-path ./plugins/simpleton-http/Cargo.toml --aggressive
  cargo update --manifest-path ./plugins/simpleton-window/Cargo.toml --aggressive
  cargo update --manifest-path ./plugins/simpleton-renderer/Cargo.toml --aggressive
  cargo update --manifest-path ./runners/simpleton/Cargo.toml --aggressive
  cargo update --manifest-path ./runners/alchemyst/Cargo.toml --aggressive
  cargo update --manifest-path ./essentials/Cargo.toml --aggressive
  cargo update --manifest-path ./demos/plugin/Cargo.toml --aggressive
  cargo update --manifest-path ./demos/custom/Cargo.toml --aggressive
  cargo update --manifest-path ./tests/Cargo.toml --aggressive
  cargo update --manifest-path ./benches/Cargo.toml --aggressive

install:
  cargo install --path ./runners/simpleton
  cargo install --path ./runners/alchemyst

publish:
  cargo publish --no-verify --manifest-path ./platform/data/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/derive/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/core/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/parser/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/plugins/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/nodes/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/ffi/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./backends/vm/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frameworks/dynamic/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frameworks/pointer/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frameworks/value/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frameworks/serde/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frameworks/text/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frameworks/arena/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frontends/serde/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frontends/assembler/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frontends/vault/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./frontends/simpleton/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./runners/simpleton/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./runners/alchemyst/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./essentials/Cargo.toml