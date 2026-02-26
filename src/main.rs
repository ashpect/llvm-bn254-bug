//! LLVM Miscompilation Demo - Multi-Field Size Test
//!
//! This demonstrates that LLVM 21.1.3 miscompiles generic eval_eq ONLY with
//! larger field sizes (256-bit) on Linux aarch64 at opt-level=3.
//!
//! Expected results on Linux aarch64:
//!   - 64-bit field:  PASSES
//!   - 128-bit field: PASSES  
//!   - 256-bit field: FAILS (this is what provekit uses)

use ark_bn254::Fr as Field256;
use ark_ff::{Field, Fp128, Fp64, MontBackend, MontConfig};

// ============================================================================
// FIELD DEFINITIONS (same as whir uses)
// ============================================================================

// 64-bit Goldilocks-like field (same as whir's Field64)
#[derive(MontConfig)]
#[modulus = "18446744069414584321"]
#[generator = "7"]
pub struct FConfig64;
pub type Field64 = Fp64<MontBackend<FConfig64, 1>>;

// 128-bit field (same as whir's Field128)
#[derive(MontConfig)]
#[modulus = "340282366920938463463374557953744961537"]
#[generator = "3"]
pub struct FConfig128;
pub type Field128 = Fp128<MontBackend<FConfig128, 2>>;

// 256-bit field: using ark_bn254::Fr (same as provekit's FieldElement)

// ============================================================================
// GENERIC EVAL_EQ (the problematic pattern)
// ============================================================================

#[inline(never)]
fn eval_eq_generic<F: Field>(accumulator: &mut [F], point: &[F], scalar: F) {
    assert_eq!(accumulator.len(), 1 << point.len());
    if let [x0, xs @ ..] = point {
        let (acc_0, acc_1) = accumulator.split_at_mut(1 << xs.len());
        let s1 = scalar * x0;
        let s0 = scalar - s1;
        eval_eq_generic(acc_0, xs, s0);
        eval_eq_generic(acc_1, xs, s1);
    } else {
        accumulator[0] += scalar;
    }
}

#[inline(never)]
fn eval_eq_reference<F: Field>(accumulator: &mut [F], point: &[F], scalar: F) {
    let n = 1 << point.len();
    let num_vars = point.len();
    assert_eq!(accumulator.len(), n);
    for i in 0..n {
        let mut contribution = scalar;
        for (j, &pj) in point.iter().enumerate() {
            let bit_pos = num_vars - 1 - j;
            let bit = (i >> bit_pos) & 1;
            if bit == 1 {
                contribution = contribution * pj;
            } else {
                contribution = contribution * (F::ONE - pj);
            }
        }
        std::hint::black_box(&contribution);
        accumulator[i] += contribution;
    }
}

// ============================================================================
// TEST HARNESS
// ============================================================================

fn generate_field_vec<F: Field>(size: usize, seed: u64) -> Vec<F> {
    let mut result = Vec::with_capacity(size);
    let base = F::from(seed);
    let mut current = base;
    for _ in 0..size {
        result.push(current);
        current = current * base + F::ONE;
    }
    result
}

fn test_field<F: Field>(name: &str, iterations: usize, dim: usize) -> usize {
    let size = 1 << dim;
    let mut failures = 0;

    for i in 0..iterations {
        let seed = i as u64 + 54321;
        let point: Vec<F> = generate_field_vec(dim, seed);
        let scalar: F = F::from(seed * 7 + 13);

        let mut acc_suspect: Vec<F> = vec![F::ZERO; size];
        let mut acc_reference: Vec<F> = vec![F::ZERO; size];

        eval_eq_generic(&mut acc_suspect, &point, scalar);
        eval_eq_reference(&mut acc_reference, &point, scalar);

        if acc_suspect != acc_reference {
            failures += 1;
        }
    }

    let status = if failures > 0 { "FAIL" } else { "PASS" };
    eprintln!(
        "  {:>12}: {:>4} ({}/{} failures)",
        name, status, failures, iterations
    );

    failures
}

fn main() {
    const ITERATIONS: usize = 100;
    const DIM: usize = 10;

    eprintln!("╔════════════════════════════════════════════════════════════╗");
    eprintln!("║         LLVM Miscompilation Demo - Field Size Test         ║");
    eprintln!("╚════════════════════════════════════════════════════════════╝");
    eprintln!();
    eprintln!(
        "Platform: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    eprintln!("Iterations: {}, Dimension: {}", ITERATIONS, DIM);
    eprintln!();
    eprintln!("Testing generic eval_eq<F: Field> with different field sizes...");
    eprintln!();

    // Test all field sizes
    let failures_64 = test_field::<Field64>("64-bit", ITERATIONS, DIM);
    let failures_128 = test_field::<Field128>("128-bit", ITERATIONS, DIM);
    let failures_256 = test_field::<Field256>("256-bit", ITERATIONS, DIM);

    eprintln!();
    eprintln!("════════════════════════════════════════════════════════════════");
    eprintln!("SUMMARY");
    eprintln!("════════════════════════════════════════════════════════════════");

    if failures_64 == 0 && failures_128 == 0 && failures_256 > 0 {
        eprintln!();
        eprintln!("✓ 64-bit and 128-bit fields: PASS");
        eprintln!("✗ 256-bit field: FAIL");
        eprintln!();
        eprintln!("CONCLUSION: LLVM miscompiles ONLY the 256-bit field arithmetic.");
        eprintln!("This explains why whir's tests (using 64-bit) pass but provekit");
        eprintln!("(using 256-bit ark_bn254::Fr) fails.");
        eprintln!();
        eprintln!("The bug is in LLVM's optimization of 256-bit Montgomery");
        eprintln!("multiplication within generic recursive functions.");
    } else if failures_64 == 0 && failures_128 == 0 && failures_256 == 0 {
        eprintln!();
        eprintln!("✓ All field sizes: PASS");
        eprintln!();
        eprintln!("No miscompilation detected on this platform.");
        eprintln!("(This is expected on macOS or with opt-level < 3)");
    } else {
        eprintln!();
        eprintln!("Unexpected failure pattern - investigate further.");
    }

    eprintln!();

    if failures_256 > 0 {
        std::process::exit(1);
    }
}
