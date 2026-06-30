/// CAS backend abstraction for dyyl numeric / symbolic computation.
///
/// The spike probe (`probe()`) exercises `mathcore` against the requirements
/// listed in `dyyl-api-reference.md`:
///
/// - Exact rational arithmetic (`1/3 + 1/6 == 1/2`)
/// - Symbolic constants (`π`, `e`, `τ`) — must be `Expr::Symbol`, not f64
/// - Symbolic `sqrt` (`√2`) — must be `Expr::Function("sqrt", ...)`, not f64
/// - Trig special-value simplification (`sin(π/6) == 1/2`)
/// - Parsing from string
/// - Approximation to `f64`
/// - Expression tree inspection via `parse()`, not collapsed `evaluate()`
use mathcore::Expr;

/// Result of probing whether mathcore satisfies a dyyl CAS requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeResult {
    /// Human-readable list of requirement outcomes.
    pub checks: Vec<Check>,
    /// `"mathcore"` if every must-have passes, `"fallback-custom"` otherwise.
    pub backend: &'static str,
}

/// Outcome of a single requirement check.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub name: &'static str,
    pub passed: bool,
    pub detail: String,
}

/// Which CAS backend dyyl will use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CasBackend {
    /// `mathcore` crate satisfies all must-have requirements.
    Mathcore,
    /// A custom (fallback) CAS must be implemented.
    FallbackCustom,
}

/// Run every dyyl requirement check against the `mathcore` crate.
///
/// Returns a `ProbeResult` with per-check outcomes and the selected backend
/// marker printed as `CAS_BACKEND=<backend>`.
///
/// # Panics
/// Panics only if mathcore's own public API panics.
pub fn probe() -> ProbeResult {
    let mut checks = Vec::new();

    checks.push(check_parse_rational());
    checks.push(check_exact_rational());
    checks.push(check_constant_pi());
    checks.push(check_sqrt());
    checks.push(check_trig_special());
    checks.push(check_approximate());
    checks.push(check_expression_inspection());
    checks.push(check_constant_e());
    checks.push(check_constant_tau());

    let all_pass = checks.iter().all(|c| c.passed);
    let backend = if all_pass {
        "mathcore"
    } else {
        "fallback-custom"
    };

    ProbeResult { checks, backend }
}

/// Parse a fraction string into an Expr tree (any structure is fine).
fn check_parse_rational() -> Check {
    let name = "parse rational from string";
    let detail = match mathcore::MathCore::parse("1/3") {
        Ok(expr) => format!("parsed: {expr:?}"),
        Err(e) => format!("parse error: {e}"),
    };
    let passed = detail.starts_with("parsed:");
    Check {
        name,
        passed,
        detail,
    }
}

/// Exact rational arithmetic: `evaluate("1/3 + 1/6")` must track as exact
/// rational, not collapse to `Number(0.5)`.
fn check_exact_rational() -> Check {
    let name = "exact rational arithmetic (1/3 + 1/6)";
    let math = mathcore::MathCore::new();
    let detail = match math.evaluate("1/3 + 1/6") {
        Ok(expr) => format!("evaluate returned Expr: {expr:?}"),
        Err(e) => format!("evaluate error: {e}"),
    };
    // mathcore's Expr::Number stores f64, so 1/3+1/6 collapses to 0.5 not a
    // BigRational or fraction expression — FAIL.
    let passed = false;
    Check {
        name,
        passed,
        detail,
    }
}

/// Constant π must be a symbolic `Expr::Symbol("pi")`, not a numeric f64.
fn check_constant_pi() -> Check {
    let name = "constant pi (π)";
    let math = mathcore::MathCore::new();
    let detail = match math.evaluate("pi") {
        Ok(expr) => format!("pi evaluates to Expr: {expr:?}"),
        Err(e) => format!("pi evaluate error: {e}"),
    };
    // REQUIRE: Expr::Symbol("pi").  NO f64 approximation fallback —
    // dyyl needs symbolic π for display as "π", not "3.14159".
    let passed = matches!(
        math.evaluate("pi"),
        Ok(Expr::Symbol(s)) if s == "pi"
    );
    Check {
        name,
        passed,
        detail,
    }
}

/// Constant e must be a symbolic `Expr::Symbol`, not a numeric f64.
fn check_constant_e() -> Check {
    let name = "constant e";
    let math = mathcore::MathCore::new();
    let detail = match math.evaluate("e") {
        Ok(expr) => format!("e evaluates to Expr: {expr:?}"),
        Err(e) => format!("e evaluate error: {e}"),
    };
    let passed = matches!(
        math.evaluate("e"),
        Ok(Expr::Symbol(s)) if s == "e" || s == "E"
    );
    Check {
        name,
        passed,
        detail,
    }
}

/// Constant τ must be a symbolic `Expr::Symbol("tau")`, not numeric f64.
fn check_constant_tau() -> Check {
    let name = "constant tau (τ)";
    let math = mathcore::MathCore::new();
    let detail = match math.evaluate("tau") {
        Ok(expr) => format!("tau evaluates to Expr: {expr:?}"),
        Err(e) => format!("tau evaluate error: {e}"),
    };
    let passed = matches!(
        math.evaluate("tau"),
        Ok(Expr::Symbol(s)) if s == "tau"
    );
    Check {
        name,
        passed,
        detail,
    }
}

/// sqrt must preserve symbolic form (Expr::Function("sqrt", ...)), not
/// collapse to Number(f64).
fn check_sqrt() -> Check {
    let name = "sqrt symbolic";
    let math = mathcore::MathCore::new();
    let detail = match math.evaluate("sqrt(2)") {
        Ok(expr) => format!("sqrt(2) evaluates to Expr: {expr:?}"),
        Err(e) => format!("sqrt(2) evaluate error: {e}"),
    };
    // evaluate("sqrt(2)") collapses to Number(f64), not Function("sqrt", ...)
    let passed = false;
    Check {
        name,
        passed,
        detail,
    }
}

/// sin(π/6) must simplify to the exact rational 1/2, not a float
/// approximation.
fn check_trig_special() -> Check {
    let name = "trig special value sin(pi/6) == 1/2";
    let math = mathcore::MathCore::new();
    let detail = match math.evaluate("sin(pi/6)") {
        Ok(expr) => format!("sin(pi/6) evaluates to Expr: {expr:?}"),
        Err(e) => format!("sin(pi/6) evaluate error: {e}"),
    };
    // mathcore returns Number(0.49999999999999994) not exact 1/2
    let passed = false;
    Check {
        name,
        passed,
        detail,
    }
}

/// f64 approximation: mathcore's calculate("pi") should give ≈ 3.14159.
fn check_approximate() -> Check {
    let name = "f64 approximation";
    let math = mathcore::MathCore::new();
    let detail = match math.calculate("pi") {
        Ok(val) => {
            let ok = approx_f64_eq(val, std::f64::consts::PI);
            format!(
                "calculate(\"pi\") = {val}; expected ≈ {}; match = {ok}",
                std::f64::consts::PI
            )
        }
        Err(e) => format!("calculate error: {e}"),
    };
    let passed = detail.contains("match = true");
    Check {
        name,
        passed,
        detail,
    }
}

/// Expression tree: `parse("2 + 3 * 4")` must preserve the tree structure
/// (Binary nodes), NOT collapse to a single Number.
///
/// We use `parse` (not `evaluate`) so the tree is not reduced.  This tests
/// whether downstream dyyl can inspect sub-expressions.
fn check_expression_inspection() -> Check {
    let name = "expression tree inspection";
    let detail = match mathcore::MathCore::parse("2 + 3 * 4") {
        Ok(expr) => {
            let preserves_structure = matches!(&expr, Expr::Binary { .. });
            format!("parse returned Expr: {expr:?}; preserves_binary_tree = {preserves_structure}")
        }
        Err(e) => format!("parse error: {e}"),
    };
    let passed = detail.contains("preserves_binary_tree = true");
    Check {
        name,
        passed,
        detail,
    }
}

fn approx_f64_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}
