// ETNA workload runner for regex-syntax.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: BlankClassCorrect | PrintRoundtripParses |
//             NamedNeqComplements | DirectiveRepRejected | All
//
// Each run emits a single JSON line on stdout; exit status is always 0 on
// completion (non-zero exit is reserved for adapter-level panics that escape
// the catch_unwind in main()).

use crabcheck::quickcheck as crabcheck_qc;
use hegel::{generators as hgen, Hegel, Settings as HegelSettings};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestRunner};
use quickcheck::{QuickCheck, ResultStatus, TestResult};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use regex_syntax::etna::{
    property_blank_class_correct, property_directive_rep_rejected,
    property_named_neq_complements, property_print_roundtrip_parses,
    PropertyResult,
};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &[
    "BlankClassCorrect",
    "PrintRoundtripParses",
    "NamedNeqComplements",
    "DirectiveRepRejected",
];

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    let mut final_status: Result<(), String> = Ok(());
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if r.is_err() && final_status.is_ok() {
            final_status = r;
        }
    }
    (final_status, total)
}

// ------------------------------------------------------------------- helpers

// Build a string whose parse may yield an empty class. The shape byte picks
// one of several empty-class-friendly patterns; the body bytes are mapped
// to lowercase ASCII letters to keep things parseable.
fn empty_class_pattern(shape: u8, b1: u8, b2: u8) -> String {
    let c1 = char::from(b'a' + (b1 % 26));
    let c2 = char::from(b'a' + (b2 % 26));
    match shape % 5 {
        0 => format!("[{}&&{}]", c1, c2),
        1 => format!("[{}-{}--{}-{}]", c1, c2, c1, c2),
        2 => format!("[{}{}]", c1, c2),
        3 => format!(r"\P{{any}}"),
        _ => format!("({}|{}){{2}}", c1, c2),
    }
}

fn map_byte_to_unicode_char(b: u8) -> char {
    // Spread the seed byte across a few interesting Unicode regions: ASCII
    // letters, ASCII whitespace/punctuation, Unicode separators, and a couple
    // of multibyte letters.
    match b % 16 {
        0 => ' ',
        1 => '\t',
        2 => '\n',
        3 => '\u{00A0}', // NO-BREAK SPACE (Zs)
        4 => '\u{1680}', // OGHAM SPACE MARK (Zs)
        5 => '\u{2028}', // LINE SEPARATOR (Zl)
        6 => '\u{2029}', // PARAGRAPH SEPARATOR (Zp)
        7 => 'A',
        8 => 'a',
        9 => '0',
        10 => '!',
        11 => 'ä',
        12 => 'Ω',
        13 => '香',
        14 => '\u{1F600}', // GRINNING FACE
        _ => '\0',
    }
}

// ----------------------------------------------------------------------- etna

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let result = match property {
        "BlankClassCorrect" => to_err(property_blank_class_correct('\n')),
        "PrintRoundtripParses" => {
            to_err(property_print_roundtrip_parses(r"[a&&b]".to_string()))
        }
        "NamedNeqComplements" => to_err(property_named_neq_complements(' ')),
        "DirectiveRepRejected" => to_err(property_directive_rep_rejected(0, 1)),
        _ => {
            return (
                Err(format!("Unknown property for etna: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    (result, Metrics { inputs: 1, elapsed_us })
}

// -------------------------------------------------------------------- proptest

fn char_strategy() -> BoxedStrategy<char> {
    any::<char>().boxed()
}

fn pattern_strategy() -> BoxedStrategy<String> {
    // A short string built from regex-meaningful characters. Bias toward
    // shapes that produce empty classes (intersection / subtraction).
    prop_oneof![
        "[a-zA-Z0-9()\\[\\]{}|*+?.^$\\\\&\\-:!]{1,10}".prop_map(String::from),
        ("[a-zA-Z]", "[a-zA-Z]").prop_map(|(a, b)| format!("[{}&&{}]", a, b)),
        ("[a-zA-Z]", "[a-zA-Z]").prop_map(|(a, b)| format!("[{}-{}--{}-{}]", a, b, a, b)),
        Just(r"\P{any}".to_string()),
        Just(r"[a&&b]".to_string()),
    ]
    .boxed()
}

fn flag_idx_n_strategy() -> BoxedStrategy<(u8, u32)> {
    (any::<u8>(), 0u32..1000).boxed()
}

fn run_proptest_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let mut runner = TestRunner::new(ProptestConfig::default());
    let c = counter.clone();
    let result: Result<(), String> = match property {
        "BlankClassCorrect" => runner
            .run(&char_strategy(), move |arg| {
                c.fetch_add(1, Ordering::Relaxed);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_blank_class_correct(arg)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({:?})", arg)))
                    }
                }
            })
            .map_err(|e| match e {
                proptest::test_runner::TestError::Fail(r, _) => r.to_string(),
                other => other.to_string(),
            }),
        "PrintRoundtripParses" => runner
            .run(&pattern_strategy(), move |arg| {
                c.fetch_add(1, Ordering::Relaxed);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_print_roundtrip_parses(arg.clone())
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({:?})", arg)))
                    }
                }
            })
            .map_err(|e| match e {
                proptest::test_runner::TestError::Fail(r, _) => r.to_string(),
                other => other.to_string(),
            }),
        "NamedNeqComplements" => runner
            .run(&char_strategy(), move |arg| {
                c.fetch_add(1, Ordering::Relaxed);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_named_neq_complements(arg)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({:?})", arg)))
                    }
                }
            })
            .map_err(|e| match e {
                proptest::test_runner::TestError::Fail(r, _) => r.to_string(),
                other => other.to_string(),
            }),
        "DirectiveRepRejected" => runner
            .run(&flag_idx_n_strategy(), move |args| {
                c.fetch_add(1, Ordering::Relaxed);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_directive_rep_rejected(args.0, args.1)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => {
                        Err(TestCaseError::fail(format!("({:?})", args)))
                    }
                }
            })
            .map_err(|e| match e {
                proptest::test_runner::TestError::Fail(r, _) => r.to_string(),
                other => other.to_string(),
            }),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = counter.load(Ordering::Relaxed);
    (result, Metrics { inputs, elapsed_us })
}

// ----------------------------------------------------------------- quickcheck
//
// quickcheck (the forked crate) takes a fn pointer, so per-property counters
// must be `static AtomicU64`. Per-fn `qc_<prop>` adapters wrap the property.

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qc_blank_class_correct(b: u8) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let c = map_byte_to_unicode_char(b);
    match property_blank_class_correct(c) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn qc_print_roundtrip_parses(shape: u8, a: u8, b: u8) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_print_roundtrip_parses(empty_class_pattern(shape, a, b)) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn qc_named_neq_complements(b: u8) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let c = map_byte_to_unicode_char(b);
    match property_named_neq_complements(c) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn qc_directive_rep_rejected(flag_idx: u8, n: u32) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_directive_rep_rejected(flag_idx, n % 1000) {
        PropertyResult::Pass => TestResult::passed(),
        PropertyResult::Discard => TestResult::discard(),
        PropertyResult::Fail(_) => TestResult::failed(),
    }
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let result = match property {
        "BlankClassCorrect" => QuickCheck::new()
            .tests(200)
            .max_tests(2000)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_blank_class_correct as fn(u8) -> TestResult),
        "PrintRoundtripParses" => QuickCheck::new()
            .tests(200)
            .max_tests(2000)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_print_roundtrip_parses as fn(u8, u8, u8) -> TestResult),
        "NamedNeqComplements" => QuickCheck::new()
            .tests(200)
            .max_tests(2000)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_named_neq_complements as fn(u8) -> TestResult),
        "DirectiveRepRejected" => QuickCheck::new()
            .tests(200)
            .max_tests(2000)
            .max_time(Duration::from_secs(86_400))
            .quicktest(qc_directive_rep_rejected as fn(u8, u32) -> TestResult),
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = QC_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("aborted: {err:?}")),
        ResultStatus::TimedOut => Err("timed out".to_string()),
        ResultStatus::GaveUp => Err(format!(
            "gave up: passed={}, discarded={}",
            result.n_tests_passed, result.n_tests_discarded
        )),
    };
    (status, metrics)
}

// ------------------------------------------------------------------ crabcheck

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn cc_blank_class_correct(b: u8) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let c = map_byte_to_unicode_char(b);
    match property_blank_class_correct(c) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_print_roundtrip_parses(args: (u8, u8, u8)) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let (shape, a, b) = args;
    match property_print_roundtrip_parses(empty_class_pattern(shape, a, b)) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_named_neq_complements(b: u8) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let c = map_byte_to_unicode_char(b);
    match property_named_neq_complements(c) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_directive_rep_rejected(args: (u8, u32)) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_directive_rep_rejected(args.0, args.1 % 1000) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cfg = crabcheck_qc::Config { tests: 2000 };
    let result = match property {
        "BlankClassCorrect" => crabcheck_qc::quickcheck_with_config(
            cfg,
            cc_blank_class_correct as fn(u8) -> Option<bool>,
        ),
        "PrintRoundtripParses" => crabcheck_qc::quickcheck_with_config(
            cfg,
            cc_print_roundtrip_parses as fn((u8, u8, u8)) -> Option<bool>,
        ),
        "NamedNeqComplements" => crabcheck_qc::quickcheck_with_config(
            cfg,
            cc_named_neq_complements as fn(u8) -> Option<bool>,
        ),
        "DirectiveRepRejected" => crabcheck_qc::quickcheck_with_config(
            cfg,
            cc_directive_rep_rejected as fn((u8, u32)) -> Option<bool>,
        ),
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {property}")),
                Metrics::default(),
            )
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = CC_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match result.status {
        crabcheck_qc::ResultStatus::Finished => Ok(()),
        crabcheck_qc::ResultStatus::Failed { arguments } => {
            Err(format!("({})", arguments.join(" ")))
        }
        crabcheck_qc::ResultStatus::TimedOut => Err("timed out".to_string()),
        crabcheck_qc::ResultStatus::GaveUp => Err(format!(
            "gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        crabcheck_qc::ResultStatus::Aborted { error } => Err(format!("aborted: {error}")),
    };
    (status, metrics)
}

// ---------------------------------------------------------------------- hegel

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    HegelSettings::new()
        .test_cases(200)
        .suppress_health_check(hegel::HealthCheck::all())
}

fn hegel_draw_unicode_char(tc: &hegel::TestCase) -> char {
    let v: u8 = tc.draw(hgen::integers::<u8>());
    map_byte_to_unicode_char(v)
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match property {
        "BlankClassCorrect" => {
            Hegel::new(|tc: hegel::TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let c = hegel_draw_unicode_char(&tc);
                let cex = format!("({:?})", c);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_blank_class_correct(c)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "PrintRoundtripParses" => {
            Hegel::new(|tc: hegel::TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let shape: u8 = tc.draw(hgen::integers::<u8>());
                let a: u8 = tc.draw(hgen::integers::<u8>());
                let b: u8 = tc.draw(hgen::integers::<u8>());
                let s = empty_class_pattern(shape, a, b);
                let cex = format!("({:?})", s);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_print_roundtrip_parses(s.clone())
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "NamedNeqComplements" => {
            Hegel::new(|tc: hegel::TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let c = hegel_draw_unicode_char(&tc);
                let cex = format!("({:?})", c);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_named_neq_complements(c)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "DirectiveRepRejected" => {
            Hegel::new(|tc: hegel::TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let flag_idx: u8 = tc.draw(hgen::integers::<u8>());
                let n_raw: u32 = tc.draw(hgen::integers::<u32>());
                let n = n_raw % 1000;
                let cex = format!("({:?}, {:?})", flag_idx, n);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_directive_rep_rejected(flag_idx, n)
                }));
                match res {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{cex}"),
                }
            })
            .settings(settings.clone())
            .run();
        }
        _ => panic!("__unknown_property:{property}"),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "hegel panicked with non-string payload".to_string()
            };
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {rest}")),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, metrics)
}

// -------------------------------------------------------------------- dispatch

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (Err(format!("Unknown tool: {tool}")), Metrics::default()),
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    metrics: Metrics,
    counterexample: Option<&str>,
    error: Option<&str>,
) {
    let cex = counterexample.map_or("null".to_string(), json_str);
    let err = error.map_or("null".to_string(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        metrics.inputs,
        json_str(&format!("{}us", metrics.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        eprintln!(
            "Properties: BlankClassCorrect | PrintRoundtripParses | \
             NamedNeqComplements | DirectiveRepRejected | All"
        );
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(tool, property)));
    std::panic::set_hook(previous_hook);

    let (status, metrics) = match caught {
        Ok(out) => out,
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "adapter panic (non-string payload)".into());
            emit_json(tool, property, "aborted", Metrics::default(), None, Some(&msg));
            return;
        }
    };
    match status {
        Ok(()) => emit_json(tool, property, "passed", metrics, None, None),
        Err(e) => emit_json(tool, property, "failed", metrics, Some(&e), None),
    }
}
