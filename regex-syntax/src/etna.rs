//! ETNA workload properties, framework-neutral.
//!
//! Each `property_<name>` is pure, deterministic, takes owned concrete inputs,
//! and returns a [`PropertyResult`]. Framework adapters (proptest, quickcheck,
//! crabcheck, hegel) and the `src/bin/etna.rs` runner all delegate to these.

use alloc::format;
use alloc::string::String;

use crate::hir::{self, Class, Hir, HirKind};
use crate::parse;

/// Three-way property outcome shared by every framework.
#[derive(Debug, Clone)]
pub enum PropertyResult {
    /// The invariant held for these inputs.
    Pass,
    /// The invariant was violated; the string carries a human-readable diagnostic.
    Fail(String),
    /// Inputs are outside the property's intended domain; treat as no-op.
    Discard,
}

fn class_contains_char(cls: &Class, c: char) -> bool {
    match cls {
        Class::Unicode(u) => u.iter().any(|r| r.start() <= c && c <= r.end()),
        Class::Bytes(b) => {
            if (c as u32) > 0xFF {
                false
            } else {
                let b8 = c as u8;
                b.iter().any(|r| r.start() <= b8 && b8 <= r.end())
            }
        }
    }
}

fn parsed_class(pattern: &str) -> Option<Class> {
    let hir = parse(pattern).ok()?;
    if let HirKind::Class(c) = hir.kind() {
        Some(c.clone())
    } else {
        None
    }
}

// ============================================================================
// blank_class — commit 5241919
// Invariant: parsing `[[:blank:]]` yields a class equivalent to {' ', '\t'}.
// Buggy version (transcription error in `ascii_class`) used range `(' ', '\t')`
// (inverted endpoints) so the class matched many more characters than intended.
// ============================================================================

/// For every `c: char`, `c` is in `[[:blank:]]` iff `c` is ASCII space or tab.
pub fn property_blank_class_correct(c: char) -> PropertyResult {
    let Some(cls) = parsed_class("[[:blank:]]") else {
        return PropertyResult::Fail("failed to parse [[:blank:]]".into());
    };
    let in_class = class_contains_char(&cls, c);
    let expected = c == ' ' || c == '\t';
    if in_class == expected {
        PropertyResult::Pass
    } else {
        PropertyResult::Fail(format!(
            "char {:?} (U+{:04X}): expected_in={} got_in={}",
            c, c as u32, expected, in_class
        ))
    }
}

#[cfg(test)]
mod blank_class_witnesses {
    use super::*;
    fn assert_pass(r: PropertyResult, label: &str) {
        match r {
            PropertyResult::Pass => {}
            PropertyResult::Discard => panic!("witness {label} unexpectedly discarded"),
            PropertyResult::Fail(m) => panic!("witness {label} failed: {m}"),
        }
    }
    #[test]
    fn witness_blank_class_correct_case_newline() {
        // '\n' is not in {' ', '\t'} but the buggy interval includes it.
        assert_pass(property_blank_class_correct('\n'), "newline");
    }
    #[test]
    fn witness_blank_class_correct_case_zero() {
        // '\x00' is not in {' ', '\t'}.
        assert_pass(property_blank_class_correct('\0'), "zero");
    }
    #[test]
    fn witness_blank_class_correct_case_uppercase_a() {
        // 'A' (0x41) is not blank under either spec.
        assert_pass(property_blank_class_correct('A'), "uppercase_a");
    }
    #[test]
    fn witness_blank_class_correct_case_space() {
        // The blank class must still contain ' '.
        assert_pass(property_blank_class_correct(' '), "space");
    }
    #[test]
    fn witness_blank_class_correct_case_tab() {
        // The blank class must still contain '\t'.
        assert_pass(property_blank_class_correct('\t'), "tab");
    }
}

// ============================================================================
// empty_class_print — commit e6d251a
// Invariant: printing an HIR (via `hir::print::Printer`) yields a regex that
// itself parses successfully — every HIR has a printable, parseable form.
// Buggy version: when the HIR's class has zero ranges, the printer emits the
// invalid string `[]` (Unicode class) or `(?-u:[])` (Bytes class). Both fail
// to re-parse: an immediate `]` after `[` is treated as a literal `]` rather
// than a class terminator.
// ============================================================================

/// `parse(print(parse(pattern)))` succeeds whenever `parse(pattern)` does.
pub fn property_print_roundtrip_parses(pattern: String) -> PropertyResult {
    let hir = match parse(&pattern) {
        Ok(h) => h,
        Err(_) => return PropertyResult::Discard,
    };
    let mut printed = String::new();
    if hir::print::Printer::new().print(&hir, &mut printed).is_err() {
        return PropertyResult::Fail("printer write error".into());
    }
    match parse(&printed) {
        Ok(_) => PropertyResult::Pass,
        Err(e) => PropertyResult::Fail(format!(
            "printed regex {:?} (from {:?}) failed to re-parse: {}",
            printed, pattern, e
        )),
    }
}

#[cfg(test)]
mod empty_class_print_witnesses {
    use super::*;
    fn assert_pass(r: PropertyResult, label: &str) {
        match r {
            PropertyResult::Pass => {}
            PropertyResult::Discard => panic!("witness {label} unexpectedly discarded"),
            PropertyResult::Fail(m) => panic!("witness {label} failed: {m}"),
        }
    }
    #[test]
    fn witness_print_roundtrip_parses_case_neg_any() {
        // `\P{any}` translates to a Unicode class with zero ranges. Buggy
        // printer emits `[]`, which fails to re-parse.
        assert_pass(
            property_print_roundtrip_parses(r"\P{any}".into()),
            "neg_any",
        );
    }
    #[test]
    fn witness_print_roundtrip_parses_case_intersect_empty() {
        // `[a&&b]` is the intersection of two disjoint singleton classes, so
        // the resulting Unicode class has zero ranges. Buggy printer emits
        // `[]` (Unicode), which fails to re-parse.
        assert_pass(
            property_print_roundtrip_parses(r"[a&&b]".into()),
            "intersect_empty",
        );
    }
    #[test]
    fn witness_print_roundtrip_parses_case_subtract_self() {
        // `[a-c--a-c]` subtracts a class from itself — also empty.
        assert_pass(
            property_print_roundtrip_parses(r"[a-c--a-c]".into()),
            "subtract_self",
        );
    }
    #[test]
    fn witness_print_roundtrip_parses_case_simple_literal() {
        // Sanity: a normal pattern roundtrips cleanly.
        assert_pass(
            property_print_roundtrip_parses("abc".into()),
            "simple_literal",
        );
    }
}

// ============================================================================
// negation_handling — commit c4865a0
// Invariant: for a Unicode property class with the `!=` operator (e.g.
// `\p{gc!=Z}`), the membership test is the complement of the same property
// with `=` — every `c: char` is in exactly one of `\p{gc=X}` and
// `\p{gc!=X}`. Buggy version delegated to `ast_class.negated` (which only
// reports whether the class uses `\p` vs `\P`), missing the `!=` operator
// embedded in the `NamedValue` AST node, so `\p{gc!=Z}` produced the same
// class as `\p{gc=Z}` rather than its negation.
// ============================================================================

/// `\p{gc=Z}` and `\p{gc!=Z}` partition the Unicode char range.
pub fn property_named_neq_complements(c: char) -> PropertyResult {
    let pos = match parse(r"\p{gc=Z}") {
        Ok(h) => h,
        Err(e) => return PropertyResult::Fail(format!("\\p{{gc=Z}} did not parse: {e}")),
    };
    let neg = match parse(r"\p{gc!=Z}") {
        Ok(h) => h,
        Err(e) => return PropertyResult::Fail(format!("\\p{{gc!=Z}} did not parse: {e}")),
    };
    let HirKind::Class(pos_cls) = pos.kind() else {
        return PropertyResult::Fail("expected class HIR for \\p{gc=Z}".into());
    };
    let HirKind::Class(neg_cls) = neg.kind() else {
        return PropertyResult::Fail("expected class HIR for \\p{gc!=Z}".into());
    };
    let in_pos = class_contains_char(pos_cls, c);
    let in_neg = class_contains_char(neg_cls, c);
    if in_pos == in_neg {
        PropertyResult::Fail(format!(
            "char {:?} (U+{:04X}) is in_pos={} in_neg={} — must differ",
            c, c as u32, in_pos, in_neg
        ))
    } else {
        PropertyResult::Pass
    }
}

#[cfg(test)]
mod negation_handling_witnesses {
    use super::*;
    fn assert_pass(r: PropertyResult, label: &str) {
        match r {
            PropertyResult::Pass => {}
            PropertyResult::Discard => panic!("witness {label} unexpectedly discarded"),
            PropertyResult::Fail(m) => panic!("witness {label} failed: {m}"),
        }
    }
    #[test]
    fn witness_named_neq_complements_case_space() {
        // ' ' (U+0020) is gc=Zs (a Separator). Buggy `\p{gc!=Z}` still
        // contains it; the fixed version excludes it.
        assert_pass(property_named_neq_complements(' '), "space");
    }
    #[test]
    fn witness_named_neq_complements_case_uppercase_a() {
        // 'A' is gc=Lu (Letter), not a Separator. The buggy `\p{gc!=Z}`
        // returns the same as `\p{gc=Z}` so excludes it; the fixed version
        // includes it.
        assert_pass(property_named_neq_complements('A'), "uppercase_a");
    }
    #[test]
    fn witness_named_neq_complements_case_no_break_space() {
        // U+00A0 NO-BREAK SPACE is also gc=Zs.
        assert_pass(property_named_neq_complements('\u{00A0}'), "nbsp");
    }
}

// ============================================================================
// counted_rep_directive — commit 7b1599f
// Invariant: a counted-repetition operator `{N}` applied to a sub-expression
// that is not a valid repetition target — specifically the empty pattern or
// a bare flag directive like `(?i)` — must return a `RepetitionMissing`
// parse error. The buggy parser accepted such inputs and produced an empty
// AST/HIR (and used to panic during HIR translation; the translator has been
// hardened, so the bug now manifests as a silent `Ok(Empty)` rather than a
// panic — but the API contract is still that this is an error).
// ============================================================================

/// `(?<flags>){N}` and other directive-only inputs followed by `{N}` must
/// return a parse error.
pub fn property_directive_rep_rejected(flag_idx: u8, n: u32) -> PropertyResult {
    let flags = match flag_idx % 8 {
        0 => "i",
        1 => "m",
        2 => "s",
        3 => "x",
        4 => "im",
        5 => "is",
        6 => "ms",
        _ => "isx",
    };
    if n > 100_000 {
        return PropertyResult::Discard;
    }
    let pat = format!("(?{}){{{}}}", flags, n);
    match parse(&pat) {
        Err(_) => PropertyResult::Pass,
        Ok(h) => PropertyResult::Fail(format!(
            "expected parse error for {:?}, got Ok({:?})",
            pat, h
        )),
    }
}

#[cfg(test)]
mod counted_rep_directive_witnesses {
    use super::*;
    fn assert_pass(r: PropertyResult, label: &str) {
        match r {
            PropertyResult::Pass => {}
            PropertyResult::Discard => panic!("witness {label} unexpectedly discarded"),
            PropertyResult::Fail(m) => panic!("witness {label} failed: {m}"),
        }
    }
    #[test]
    fn witness_directive_rep_rejected_case_i_one() {
        assert_pass(property_directive_rep_rejected(0, 1), "i_one");
    }
    #[test]
    fn witness_directive_rep_rejected_case_m_one_one() {
        // `(?m){1,1}` — same shape with explicit-min repetition — exercised
        // via the same `n=1` path; we use a different flag combination to
        // diversify witnesses.
        assert_pass(property_directive_rep_rejected(1, 1), "m_one_one");
    }
    #[test]
    fn witness_directive_rep_rejected_case_x_zero() {
        // `(?x){0}` — the zero-count case from the original regression.
        assert_pass(property_directive_rep_rejected(3, 0), "x_zero");
    }
    #[test]
    fn witness_directive_rep_rejected_case_isx_five() {
        assert_pass(property_directive_rep_rejected(7, 5), "isx_five");
    }
}

// suppress unused warnings for items only referenced by the runner crate
#[allow(dead_code)]
fn _silence_unused() {
    let _h: Option<Hir> = None;
    let _h2: Option<hir::Class> = None;
}
