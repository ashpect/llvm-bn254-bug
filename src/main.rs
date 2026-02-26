//! LLVM Miscompilation - Is it the recursive mul-sub pattern?

use ark_ff::{AdditiveGroup, Field, Fp256, MontBackend, MontConfig};
use std::hint::black_box;

#[derive(MontConfig)]
#[modulus = "21888242871839275222246405745257275088548364400416034343698204186575808495617"]
#[generator = "5"]
pub struct BN254Config;
pub type F = Fp256<MontBackend<BN254Config, 4>>;

// ============================================================================
// TEST 1: Recursive with just ADD (no mul, no sub)
// ============================================================================

#[inline(never)]
fn recurse_add_only<F: Field>(acc: &mut [F], depth: usize, scalar: F) {
    if depth == 0 {
        acc[0] += scalar;
    } else {
        let (a0, a1) = acc.split_at_mut(acc.len() / 2);
        recurse_add_only(a0, depth - 1, scalar);
        recurse_add_only(a1, depth - 1, scalar);
    }
}

// ============================================================================
// TEST 2: Recursive with MUL only (no sub)
// ============================================================================

#[inline(never)]
fn recurse_mul_only<F: Field>(acc: &mut [F], point: &[F], scalar: F) {
    if let [x0, xs @ ..] = point {
        let (a0, a1) = acc.split_at_mut(1 << xs.len());
        let s1 = scalar * x0;
        recurse_mul_only(a0, xs, scalar);  // pass original
        recurse_mul_only(a1, xs, s1);       // pass multiplied
    } else {
        acc[0] += scalar;
    }
}

// ============================================================================
// TEST 3: Recursive with SUB only (no mul)
// ============================================================================

#[inline(never)]
fn recurse_sub_only<F: Field>(acc: &mut [F], point: &[F], scalar: F) {
    if let [x0, xs @ ..] = point {
        let (a0, a1) = acc.split_at_mut(1 << xs.len());
        let s0 = scalar - *x0;
        recurse_sub_only(a0, xs, s0);       // pass subtracted
        recurse_sub_only(a1, xs, scalar);   // pass original
    } else {
        acc[0] += scalar;
    }
}

// ============================================================================
// TEST 4: Recursive with MUL then SUB (the problematic pattern)
// ============================================================================

#[inline(never)]
fn recurse_mul_sub<F: Field>(acc: &mut [F], point: &[F], scalar: F) {
    if let [x0, xs @ ..] = point {
        let (a0, a1) = acc.split_at_mut(1 << xs.len());
        let s1 = scalar * x0;      // MUL
        let s0 = scalar - s1;      // SUB using result of MUL
        recurse_mul_sub(a0, xs, s0);
        recurse_mul_sub(a1, xs, s1);
    } else {
        acc[0] += scalar;
    }
}

// ============================================================================
// Reference implementation
// ============================================================================

#[inline(never)]
fn reference<F: Field>(acc: &mut [F], point: &[F], scalar: F) {
    let n = 1 << point.len();
    for i in 0..n {
        let mut contribution = scalar;
        for (j, &pj) in point.iter().enumerate() {
            let bit = (i >> (point.len() - 1 - j)) & 1;
            if bit == 1 {
                contribution = contribution * pj;
            } else {
                contribution = contribution * (F::ONE - pj);
            }
        }
        black_box(&contribution);
        acc[i] += contribution;
    }
}

fn gen_vec(size: usize, seed: u64) -> Vec<F> {
    let base = F::from(seed);
    let mut current = base;
    (0..size).map(|_| { let v = current; current = current * base + F::ONE; v }).collect()
}

fn main() {
    const ITERS: usize = 100;
    const DIM: usize = 10;
    let size = 1 << DIM;
    
    println!("=== Isolating the Recursive Pattern ===");
    println!("Platform: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!();

    // Test 1: Add only
    let mut fails = 0;
    for i in 0..ITERS {
        let scalar = F::from(i as u64 + 1);
        let mut acc = vec![F::ZERO; size];
        recurse_add_only(&mut acc, DIM, scalar);
        // Each cell should have scalar added once
        let expected = scalar;
        if acc.iter().any(|&x| x != expected) { fails += 1; }
    }
    println!("Recursive ADD only:              {} ({}/{})", if fails > 0 { "FAIL" } else { "PASS" }, fails, ITERS);

    // Test 2: Mul only - compare with reference-like
    let mut fails = 0;
    for i in 0..ITERS {
        let point: Vec<F> = gen_vec(DIM, i as u64 + 2);
        let scalar = F::from(i as u64 + 2000);
        let mut acc1 = vec![F::ZERO; size];
        let mut acc2 = vec![F::ZERO; size];
        recurse_mul_only(&mut acc1, &point, scalar);
        // Manual reference for mul-only pattern
        for j in 0..size {
            let mut c = scalar;
            for (k, &pj) in point.iter().enumerate() {
                let bit = (j >> (DIM - 1 - k)) & 1;
                if bit == 1 { c = c * pj; }
            }
            acc2[j] = c;
        }
        if acc1 != acc2 { fails += 1; }
    }
    println!("Recursive MUL only:              {} ({}/{})", if fails > 0 { "FAIL" } else { "PASS" }, fails, ITERS);

    // Test 3: Sub only
    let mut fails = 0;
    for i in 0..ITERS {
        let point: Vec<F> = gen_vec(DIM, i as u64 + 3);
        let scalar = F::from(i as u64 + 3000);
        let mut acc1 = vec![F::ZERO; size];
        let mut acc2 = vec![F::ZERO; size];
        recurse_sub_only(&mut acc1, &point, scalar);
        // Manual reference for sub-only pattern  
        for j in 0..size {
            let mut c = scalar;
            for (k, &pj) in point.iter().enumerate() {
                let bit = (j >> (DIM - 1 - k)) & 1;
                if bit == 0 { c = c - pj; }
            }
            acc2[j] = c;
        }
        if acc1 != acc2 { fails += 1; }
    }
    println!("Recursive SUB only:              {} ({}/{})", if fails > 0 { "FAIL" } else { "PASS" }, fails, ITERS);

    // Test 4: Mul then Sub (the eval_eq pattern)
    let mut fails = 0;
    for i in 0..ITERS {
        let point: Vec<F> = gen_vec(DIM, i as u64 + 4);
        let scalar = F::from(i as u64 + 4000);
        let mut acc1 = vec![F::ZERO; size];
        let mut acc2 = vec![F::ZERO; size];
        recurse_mul_sub(&mut acc1, &point, scalar);
        reference(&mut acc2, &point, scalar);
        if acc1 != acc2 { fails += 1; }
    }
    println!("Recursive MUL then SUB:          {} ({}/{})", if fails > 0 { "FAIL" } else { "PASS" }, fails, ITERS);

    println!();
    println!("If only \"MUL then SUB\" fails, the bug is in the");
    println!("combination: s1 = scalar * x0; s0 = scalar - s1");
}
