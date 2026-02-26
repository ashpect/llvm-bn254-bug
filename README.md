# LLVM Miscompilation Bug

LLVM 21.1.3 miscompiles this pattern on Linux aarch64:

```rust
let s1 = scalar * x0;      // MUL
let s0 = scalar - s1;      // SUB result of MUL
```

## Reproduce

```bash
cargo run            # PASS (debug)
cargo run --release  # FAIL (opt-level=3 + codegen-units=1)
```

## Conditions

- `opt-level=3` + `codegen-units=1`
- 256-bit field (64/128-bit work fine)
- Generic recursive function
- Linux aarch64 only (macOS aarch64 works)

## Fix

```toml
[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "codegen-units=16"]
```
