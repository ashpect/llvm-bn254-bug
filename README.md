# LLVM 21.1.3 Miscompilation Bug on Linux aarch64

This repository demonstrates a compiler bug in LLVM 21.1.3 that causes incorrect code generation for 256-bit Montgomery field arithmetic on Linux aarch64.

## TL;DR

```bash
cargo run            # ✅ Debug mode passes
cargo run --release  # ❌ Release mode fails (100% of the time)
```

The bug is triggered by the combination of:
- `opt-level=3`
- `codegen-units=1`  
- 256-bit field arithmetic (64-bit and 128-bit work fine)
- Generic functions with recursive patterns

## The Bug

LLVM miscompiles this specific pattern in recursive generic functions:

```rust
fn recurse<F: Field>(acc: &mut [F], point: &[F], scalar: F) {
    if let [x0, xs @ ..] = point {
        let (a0, a1) = acc.split_at_mut(1 << xs.len());
        let s1 = scalar * x0;      // MUL
        let s0 = scalar - s1;      // SUB result of MUL  <-- BUG HERE
        recurse(a0, xs, s0);
        recurse(a1, xs, s1);
    } else {
        acc[0] += scalar;
    }
}
```

**Key finding**: Individual operations (MUL, SUB, ADD) work fine in isolation. Only the combination of `MUL then SUB` in a recursive context fails.

## Isolation Test Results

| Pattern | Result |
|---------|--------|
| Recursive ADD only | ✅ PASS |
| Recursive MUL only | ✅ PASS |
| Recursive SUB only | ✅ PASS |
| **Recursive MUL then SUB** | ❌ **FAIL** |

## Conditions Matrix

| Field Size | Linux aarch64 | macOS aarch64 |
|------------|---------------|---------------|
| 64-bit     | ✅ Pass       | ✅ Pass       |
| 128-bit    | ✅ Pass       | ✅ Pass       |
| 256-bit    | ❌ **Fail**   | ✅ Pass       |

| opt-level | codegen-units | Result |
|-----------|---------------|--------|
| 3         | 16 (default)  | ✅ Pass |
| 3         | 1             | ❌ Fail |
| 2         | 1             | ✅ Pass |

## Reproduce

### On Linux aarch64

```bash
git clone https://github.com/ashpect/llvm-bn254-bug
cd llvm-bn254-bug
cargo run --release
```

Expected output:
```
=== Isolating the Recursive Pattern ===
Platform: linux aarch64

Recursive ADD only:              PASS (0/100)
Recursive MUL only:              PASS (0/100)
Recursive SUB only:              PASS (0/100)
Recursive MUL then SUB:          FAIL (100/100)

If only "MUL then SUB" fails, the bug is in the
combination: s1 = scalar * x0; s0 = scalar - s1
```

### On macOS aarch64 (for comparison)

All tests pass - this is a Linux-specific issue.

## The Fix

Add to `.cargo/config.toml`:

```toml
[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "opt-level=2"]
```

This reduces optimization level only for Linux aarch64, avoiding the buggy code generation while maintaining performance on other platforms.

## Environment

- **Rust**: 1.87.0
- **LLVM**: 21.1.3
- **Target**: aarch64-unknown-linux-gnu
- **Field**: BN254 Fr (256-bit Montgomery form, 4 limbs)
- **Tested on**: GCP c4a instances (Axion/Neoverse V2)

## Real-World Impact

This bug affects the [WHIR](https://github.com/WizardOfMenlo/whir) zero-knowledge proof library's `eval_eq` function, causing proof verification to fail on Linux ARM servers (AWS Graviton, GCP Axion, etc.).

The `eval_eq` function uses exactly the problematic pattern - it's a core building block for multilinear polynomial evaluation in the proving system.

## Why Wasn't This Caught Earlier?

WHIR's test suite uses 64-bit test fields (`Field64`) for speed. The bug only manifests with 256-bit fields (like `ark_bn254::Fr`) used in production cryptographic applications.

## Related

- [provekit issue #304](https://github.com/worldfnd/provekit/issues/304) - Original bug report
- [ark-ff](https://github.com/arkworks-rs/algebra) - Field arithmetic library affected
