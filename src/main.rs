//! LLVM 21.1.3 miscompiles this on Linux aarch64 with opt-level=3 + codegen-units=1

use ark_ff::{AdditiveGroup, Field, Fp256, MontBackend, MontConfig};

#[derive(MontConfig)]
#[modulus = "21888242871839275222246405745257275088548364400416034343698204186575808495617"]
#[generator = "5"]
pub struct BN254Config;
pub type F = Fp256<MontBackend<BN254Config, 4>>;

/// The buggy pattern: MUL then SUB in recursive generic function
#[inline(never)]
fn eval_eq<F: Field>(acc: &mut [F], point: &[F], scalar: F) {
    if let [x0, xs @ ..] = point {
        let (a0, a1) = acc.split_at_mut(1 << xs.len());
        let s1 = scalar * x0;      // MUL
        let s0 = scalar - s1;      // SUB result of MUL <- LLVM miscompiles this
        eval_eq(a0, xs, s0);
        eval_eq(a1, xs, s1);
    } else {
        acc[0] += scalar;
    }
}

/// Reference: iterative, no optimization opportunity for LLVM to break
#[inline(never)]
fn reference<F: Field>(acc: &mut [F], point: &[F], scalar: F) {
    for i in 0..(1 << point.len()) {
        let mut c = scalar;
        for (j, &pj) in point.iter().enumerate() {
            if (i >> (point.len() - 1 - j)) & 1 == 1 {
                c = c * pj;
            } else {
                c = c * (F::ONE - pj);
            }
        }
        acc[i] += c;
    }
}

fn main() {
    let point: Vec<F> = (1..=10).map(|i| F::from(i * 7)).collect();
    let scalar = F::from(12345u64);
    let size = 1 << point.len();

    let mut recursive = vec![F::ZERO; size];
    let mut iterative = vec![F::ZERO; size];

    eval_eq(&mut recursive, &point, scalar);
    reference(&mut iterative, &point, scalar);

    if recursive == iterative {
        println!("PASS");
    } else {
        println!("FAIL - LLVM miscompiled eval_eq");
        let mismatches = recursive.iter().zip(&iterative).filter(|(a, b)| a != b).count();
        println!("{}/{} values wrong", mismatches, size);
    }
}
