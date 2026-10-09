//! Real Lean tests; run after setup with: cargo test --test lean -- --ignored
use recurrence_lab::{
    generator,
    math::Expr,
    verifier::{VerificationError, Verifier},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
fn verifier() -> Verifier {
    Verifier::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean"))
}
#[test]
#[ignore = "requires prepared Lean 4.19 + mathlib"]
fn every_registered_family_has_audited_universal_proof() {
    let verifier = verifier();
    assert!(verifier.status().ready, "{}", verifier.status().message);
    for family in generator::family_catalog() {
        let p = generator::generate(family.levels[0], Some(&family.id), 20261009).unwrap();
        let result = verifier.verify(
            &p,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(60),
        );
        assert!(result.is_ok(), "{}: {:?}", family.id, result);
        let record = result.unwrap();
        assert_eq!(record.problem_hash, p.id);
        assert_eq!(record.checked_theorems.len(), 4);
        println!("{}: Lean verified {} ms", family.id, record.elapsed_ms);
    }
}
#[test]
#[ignore = "requires prepared Lean 4.19 + mathlib"]
fn changed_problem_timeout_and_cancel_cannot_be_successful() {
    let verifier = verifier();
    assert!(verifier.status().ready);
    let p = generator::generate(1, Some("arithmetic"), 32).unwrap();
    let mut changed = p.clone();
    changed.problem.general_term.rhs = Expr::integer(999);
    assert!(matches!(
        verifier.verify(
            &changed,
            Arc::new(AtomicBool::new(false)),
            Duration::from_secs(60)
        ),
        Err(VerificationError::Failed(_))
    ));
    assert!(matches!(
        verifier.verify(
            &p,
            Arc::new(AtomicBool::new(false)),
            Duration::from_millis(1)
        ),
        Err(VerificationError::Timeout)
    ));
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let v = verifier.clone();
    let worker = thread::spawn(move || v.verify(&p, flag, Duration::from_secs(60)));
    thread::sleep(Duration::from_millis(100));
    cancelled.store(true, Ordering::SeqCst);
    assert!(matches!(
        worker.join().unwrap(),
        Err(VerificationError::Cancelled)
    ));
}
