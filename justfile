all:
  @echo "Select target"

test:
  cargo test -F serde

bump:
  cargo bump

pub:
  cargo publish
