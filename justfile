list:
  just --list

format:
  cargo fmt --all

build:
  cargo build --all --all-features

test:
  cargo test --all --all-features

bench:
  cargo test --manifest-path ./tests/Cargo.toml --features=bench --release -- --nocapture

clippy:
  cargo clippy --all --all-features

checks:
  just build
  just test
  just clippy

demo:
  cd ./demos/tetra/ && cargo run --release

clean:
  find . -name target -type d -exec rm -r {} +

remove-lockfiles:
  find . -name Cargo.lock -type f -exec rm {} +

list-outdated:
  cargo outdated -R -w

update:
  cargo update --workspace

install:
  cargo install --path ./runners/simpleton

publish:
  cargo publish --no-verify --manifest-path ./data/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./core/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./derive/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./backends/vm/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./backends/rust/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./frontends/serde/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./frontends/vault/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./frontends/simpleton/Cargo.toml
  sleep 15
  cargo publish --no-verify --manifest-path ./runners/simpleton/Cargo.toml