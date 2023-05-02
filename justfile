list:
  just --list

format:
  cargo fmt --all

build:
  cargo build --all --all-features

test:
  cargo test --all --all-features
  cd ./runners/simpleton && just run
  cd ./runners/alchemyst && just run

bench:
  cargo test --manifest-path ./tests/Cargo.toml --features=bench --release -- --nocapture

clippy:
  cargo clippy --all --all-features

checks:
  just build
  just clippy
  just test

demo:
  cd ./demos/tetra/ && cargo run --release

demo-node-graph:
  cargo build --release --manifest-path ./demos/godot-node-graph/server/Cargo.toml
  godot --path ./demos/godot-node-graph/editor --export "Windows Desktop" ../../../target/release/godot-node-graph.exe
  ./target/release/godot-node-graph.exe

clean:
  find . -name target -type d -exec rm -r {} +
  just remove-lockfiles

remove-lockfiles:
  find . -name Cargo.lock -type f -exec rm {} +

list-outdated:
  cargo outdated -R -w

update:
  cargo update --manifest-path ./platform/data/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/core/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/plugins/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/derive/Cargo.toml --aggressive
  cargo update --manifest-path ./platform/nodes/Cargo.toml --aggressive
  cargo update --manifest-path ./backends/vm/Cargo.toml --aggressive
  cargo update --manifest-path ./backends/rust/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/serde/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/assembler/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/simpleton/Cargo.toml --aggressive
  cargo update --manifest-path ./frontends/vault/Cargo.toml --aggressive
  cargo update --manifest-path ./plugins/simpleton-http/Cargo.toml --aggressive
  cargo update --manifest-path ./runners/simpleton/Cargo.toml --aggressive
  cargo update --manifest-path ./runners/alchemyst/Cargo.toml --aggressive
  cargo update --manifest-path ./essentials/Cargo.toml --aggressive
  cargo update --manifest-path ./demos/tetra/Cargo.toml --aggressive
  cargo update --manifest-path ./demos/plugin/Cargo.toml --aggressive
  cargo update --manifest-path ./demos/emu/Cargo.toml --aggressive
  cargo update --manifest-path ./demos/custom/Cargo.toml --aggressive
  cargo update --manifest-path ./tests/Cargo.toml --aggressive

install:
  cargo install --path ./runners/simpleton
  cargo install --path ./runners/alchemyst

publish:
  cargo publish --no-verify --manifest-path ./platform/data/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/core/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/plugins/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/derive/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./platform/nodes/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./backends/vm/Cargo.toml
  sleep 1
  cargo publish --no-verify --manifest-path ./backends/rust/Cargo.toml
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