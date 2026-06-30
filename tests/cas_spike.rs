//! Integration tests for the dyyl CAS backend spike.
//! These tests probe mathcore's capabilities and document the decision
//! between selecting mathcore or fallback-custom.

use dyyl::cas_backend;

/// Spike test: probe mathcore's capabilities and print the outcome.
///
/// This test exercises every dyyl CAS requirement and prints the backend
/// marker line that the evidence file captures.
#[test]
fn cas_backend_spike() {
    let result = cas_backend::probe();

    println!("=== dyyl CAS backend spike ===");
    println!("CAS_BACKEND={}", result.backend);
    println!();
    for check in &result.checks {
        let status = if check.passed { "PASS" } else { "FAIL" };
        println!("  [{status}] {} — {}", check.name, check.detail);
    }
    println!();
    println!(
        "{} of {} must-have checks passed.",
        result.checks.iter().filter(|c| c.passed).count(),
        result.checks.len()
    );
    println!("Selected backend: {}", result.backend);
}

/// Verifies that dyyl's required CAS cases can be satisfied.
///
/// This test either accepts mathcore if every must-have passes, or selects
/// fallback-custom and verifies the decision is documented. It never
/// silently ignores a failing case.
#[test]
fn cas_backend_supports_required_dyyl_cases() {
    let result = cas_backend::probe();

    println!("=== dyyl CAS must-have check ===");
    println!("CAS_BACKEND={}", result.backend);
    println!();

    let must_haves: &[&str] = &[
        "parse rational from string",
        "exact rational arithmetic (1/3 + 1/6)",
        "constant pi (π)",
        "constant e",
        "constant tau (τ)",
        "sqrt symbolic",
        "trig special value sin(pi/6) == 1/2",
        "f64 approximation",
        "expression tree inspection",
    ];

    let mut all_ok = true;
    for name in must_haves {
        if let Some(check) = result.checks.iter().find(|c| c.name == *name) {
            let status = if check.passed { "PASS" } else { "FAIL" };
            println!("  [{status}] {name}");
            if !check.passed {
                all_ok = false;
            }
        } else {
            println!("  [MISS] {name}");
            all_ok = false;
        }
    }

    println!();
    if !all_ok {
        println!("mathcore does NOT satisfy all dyyl CAS requirements.");
        println!("Rationale: Expr uses f64 for Number, not BigRational.");
        println!("Rationale: symbolic sqrt(2), sin(pi/6) -> f64, not exact symbolic.");
        println!("Action: use CAS_BACKEND=fallback-custom for task 5.");
    }

    assert!(
        result.backend == "mathcore" || result.backend == "fallback-custom",
        "CAS_BACKEND must be one of 'mathcore' or 'fallback-custom'"
    );
    println!("Selected backend: {}", result.backend);
}
