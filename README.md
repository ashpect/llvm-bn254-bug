# LLVM Miscompilation Demo

LLVM 21.1.3 miscompiles generic 256-bit field arithmetic on Linux aarch64 at opt-level=3.

## Run

```bash
cargo build --release
./target/release/llvm-bug-demo
```

## Results (Linux aarch64)

```
    64-bit: PASS 
   128-bit: PASS 
   256-bit: FAIL 
```

## Fix

```toml
# .cargo/config.toml
[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "opt-level=2"]
```
