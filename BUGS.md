# regex-syntax — Injected Bugs

regex-syntax — the regular expression parser used by the `regex` crate. ETNA workload mining bug-fix commits in `rust-lang/regex` that touched the regex-syntax sub-crate.

Total mutations: 4

## Bug Index

| # | Variant | Name | Location | Injection | Fix Commit |
|---|---------|------|----------|-----------|------------|
| 1 | `blank_class_5241919_1` | `blank_class` | `regex-syntax/src/hir/translate.rs:1326` | `patch` | `5241919f48c3bf54fbe8d4e9d50b4a55038da150` |
| 2 | `counted_rep_directive_7b1599f_1` | `counted_rep_directive` | `regex-syntax/src/ast/parse.rs:1117` | `patch` | `7b1599f2f6903e4087c6cd109404be4056406ad9` |
| 3 | `empty_class_print_e6d251a_1` | `empty_class_print` | `regex-syntax/src/hir/print.rs:134` | `patch` | `e6d251a26090b02c43c19d5d0ee3de0838abb6e4` |
| 4 | `negation_handling_c4865a0_1` | `negation_handling` | `regex-syntax/src/hir/translate.rs:1050` | `patch` | `c4865a0c8446a701e10b0fd987f19068f5dcc365` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `blank_class_5241919_1` | `BlankClassCorrect` | `witness_blank_class_correct_case_newline`, `witness_blank_class_correct_case_zero`, `witness_blank_class_correct_case_uppercase_a`, `witness_blank_class_correct_case_space`, `witness_blank_class_correct_case_tab` |
| `counted_rep_directive_7b1599f_1` | `DirectiveRepRejected` | `witness_directive_rep_rejected_case_i_one`, `witness_directive_rep_rejected_case_m_one_one`, `witness_directive_rep_rejected_case_x_zero`, `witness_directive_rep_rejected_case_isx_five` |
| `empty_class_print_e6d251a_1` | `PrintRoundtripParses` | `witness_print_roundtrip_parses_case_neg_any`, `witness_print_roundtrip_parses_case_intersect_empty`, `witness_print_roundtrip_parses_case_subtract_self`, `witness_print_roundtrip_parses_case_simple_literal` |
| `negation_handling_c4865a0_1` | `NamedNeqComplements` | `witness_named_neq_complements_case_space`, `witness_named_neq_complements_case_uppercase_a`, `witness_named_neq_complements_case_no_break_space` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `BlankClassCorrect` | ✓ | ✓ | ✓ | ✓ |
| `DirectiveRepRejected` | ✓ | ✓ | ✓ | ✓ |
| `PrintRoundtripParses` | ✓ | ✓ | ✓ | ✓ |
| `NamedNeqComplements` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. blank_class

- **Variant**: `blank_class_5241919_1`
- **Location**: `regex-syntax/src/hir/translate.rs:1326` (inside `ascii_class`)
- **Property**: `BlankClassCorrect`
- **Witness(es)**:
  - `witness_blank_class_correct_case_newline` — Newline (U+000A) lies inside the inverted range [0x09, 0x20] but is not blank.
  - `witness_blank_class_correct_case_zero` — U+0000 — confirms control bytes do not leak into the class.
  - `witness_blank_class_correct_case_uppercase_a` — U+0041 — outside the inverted range; both versions agree.
  - `witness_blank_class_correct_case_space` — Positive case: ' ' must remain in the class.
  - `witness_blank_class_correct_case_tab` — Positive case: '\t' must remain in the class.
- **Source**: [#533](https://github.com/rust-lang/regex/issues/533) — syntax: fix [[:blank:]] character class
  > When `regex-syntax` was rewritten, a transcription error in the ASCII `Blank` class table left the entry as `[(' ', '\t')]` (a single inverted-range tuple) instead of `[('\t', '\t'), (' ', ' ')]`. Because the class normalizer expanded that into the byte interval `[0x09, 0x20]`, the POSIX `[[:blank:]]` class matched control characters and most punctuation in addition to the intended `[ \t]`.
- **Fix commit**: `5241919f48c3bf54fbe8d4e9d50b4a55038da150` — syntax: fix [[:blank:]] character class
- **Invariant violated**: Parsing `[[:blank:]]` produces a character class that contains exactly the two ASCII characters U+0020 (space) and U+0009 (tab) — `(c == ' ' || c == '\t')` is equivalent to `c` being a member of the parsed class.
- **How the mutation triggers**: The mutation reverts the `Blank` arm of `ascii_class` to the buggy `&[(b' ', b'\t')]` (inverted range). After class normalisation that becomes `[0x09, 0x20]`, and every character in the `\t..=' '` ASCII range — newline, vertical tab, form feed, carriage return, control bytes, and many printable punctuation marks — is reported as belonging to `[[:blank:]]`.

### 2. counted_rep_directive

- **Variant**: `counted_rep_directive_7b1599f_1`
- **Location**: `regex-syntax/src/ast/parse.rs:1117` (inside `ParserI::parse_counted_repetition`)
- **Property**: `DirectiveRepRejected`
- **Witness(es)**:
  - `witness_directive_rep_rejected_case_i_one` — `(?i){1}` — the original regression input.
  - `witness_directive_rep_rejected_case_m_one_one` — `(?m){1,1}` — explicit-min counted form from the regression test.
  - `witness_directive_rep_rejected_case_x_zero` — `(?x){0}` — zero-count edge case (also from the regression test).
  - `witness_directive_rep_rejected_case_isx_five` — `(?isx){5}` — multi-flag directive followed by larger count.
- **Source**: [#555](https://github.com/rust-lang/regex/issues/555) — syntax: fix counted repetition bug
  > `parse_counted_repetition` popped the previous AST off the concatenation and applied `{N}` to it without checking that the popped node was a valid repetition target. For inputs like `(?i){1}` or `(?m){1,1}` — where the node is `Ast::Flags` (a bare flag directive) — or for `(?:){5}` — where the node is `Ast::Empty` — the parser silently produced an `Ast::Repetition` over a non-expression. At the time, the HIR translator panicked on this shape; today the translator no-ops on `Repetition(Empty)` and the bug shows up as the parser quietly accepting a malformed pattern instead of returning `RepetitionMissing`. The fix adds an explicit `Ast::Empty(_) | Ast::Flags(_) => Err(RepetitionMissing)` arm before consuming the count.
- **Fix commit**: `7b1599f2f6903e4087c6cd109404be4056406ad9` — syntax: fix counted repetition bug
- **Invariant violated**: Counted repetition `{N}` (and `{N,M}`) requires a real preceding sub-expression. A pattern like `(?<flags>){N}`, where the only node before `{N}` is a flag directive that contributes no matchable expression, must be rejected with `ErrorKind::RepetitionMissing`.
- **How the mutation triggers**: The mutation removes the `Ast::Empty(_) | Ast::Flags(_)` early-return guard from `parse_counted_repetition`. The parser then consumes `{N}` and produces `Ast::Repetition(Ast::Flags(...), N)` without complaint. After HIR translation that flattens to `Ok(Empty)` instead of the expected `Err(RepetitionMissing)`, so the API silently accepts a malformed regex.

### 3. empty_class_print

- **Variant**: `empty_class_print_e6d251a_1`
- **Location**: `regex-syntax/src/hir/print.rs:134` (inside `Writer::visit_pre`)
- **Property**: `PrintRoundtripParses`
- **Witness(es)**:
  - `witness_print_roundtrip_parses_case_neg_any` — `\P{any}` is the canonical empty Unicode class.
  - `witness_print_roundtrip_parses_case_intersect_empty` — `[a&&b]` — intersection of two disjoint singletons.
  - `witness_print_roundtrip_parses_case_subtract_self` — `[a-c--a-c]` — class minus itself, also empty.
  - `witness_print_roundtrip_parses_case_simple_literal` — Sanity check: a normal pattern still roundtrips.
- **Source**: syntax: fix empty char class bug in HIR printer
  > When the HIR is a character class with zero ranges (produced by patterns like `\P{any}`, `[a&&b]`, or `[a-c--a-c]`), the HIR printer used to emit the literal string `[]` (Unicode class) or `(?-u:[])` (Bytes class). Both are unparseable: an immediate `]` after `[` is interpreted as a literal `]`, leaving the class unclosed. The fix special-cases empty classes and emits `[a&&b]`, the intersection of two disjoint singletons, which is a valid empty class.
- **Fix commit**: `e6d251a26090b02c43c19d5d0ee3de0838abb6e4` — syntax: fix empty char class bug in HIR printer
- **Invariant violated**: For every regex pattern `p` accepted by the parser, `parse(print(parse(p)))` must also succeed — every printable HIR re-parses. Empty character classes are a legal HIR shape and the printer must produce a parseable surface form for them.
- **How the mutation triggers**: The mutation removes the `cls.ranges().is_empty()` guards from both the Unicode and Bytes branches of `Writer::visit_pre`'s class arm. Empty classes then fall through to the normal `write_str("[")?; for range in cls.iter() { ... } write_str("]")?;` path, which emits `[]` (or `(?-u:[])` for bytes) — both unclosed-character-class errors when re-parsed.

### 4. negation_handling

- **Variant**: `negation_handling_c4865a0_1`
- **Location**: `regex-syntax/src/hir/translate.rs:1050` (inside `TranslatorI::hir_unicode_class`)
- **Property**: `NamedNeqComplements`
- **Witness(es)**:
  - `witness_named_neq_complements_case_space` — U+0020 is gc=Zs; under the bug `\p{gc!=Z}` mistakenly contains it.
  - `witness_named_neq_complements_case_uppercase_a` — 'A' is gc=Lu; under the bug `\p{gc!=Z}` excludes it (it equals gc=Z).
  - `witness_named_neq_complements_case_no_break_space` — U+00A0 NO-BREAK SPACE is also gc=Zs — confirms the bug isn't ASCII-specific.
- **Source**: syntax: fix negation handling in HIR translation
  > When translating an `ast::ClassUnicode` of the `NamedValue { op: NotEqual, .. }` shape (e.g. `\p{gc!=Z}`), the translator passed `ast_class.negated` to `unicode_fold_and_negate` — but `negated` only reports whether the class uses `\p` vs `\P`, not whether the embedded `!=` operator should also flip the polarity. The bug therefore made `\p{gc!=Z}` translate to the same class as `\p{gc=Z}`. The fix calls `ast_class.is_negated()`, which combines both signals.
- **Fix commit**: `c4865a0c8446a701e10b0fd987f19068f5dcc365` — syntax: fix negation handling in HIR translation
- **Invariant violated**: For every Unicode general-category property `X` and every `c: char`, `c` is in the class produced by `\p{gc=X}` exactly when it is NOT in the class produced by `\p{gc!=X}`. The two classes partition the Unicode codepoint space.
- **How the mutation triggers**: The mutation replaces `ast_class.is_negated()` with `ast_class.negated`. Because `\p{gc!=Z}` uses `\p` (not `\P`), `negated` is `false` — the `!=` operator is silently dropped. The translated class for `\p{gc!=Z}` then equals the class for `\p{gc=Z}`, so a Separator like ' ' is reported as belonging to both `\p{gc=Z}` and `\p{gc!=Z}`, violating the partition invariant.

## Dropped Candidates

- `f9aec41` (syntax: fix overflow for big counted repetitions) — concat-loop overflow only fires when usize is 32-bit; on the 64-bit test platform the buggy `*minimum_len += len` cannot reach `usize::MAX` from a feasible regex pattern, so no witness can deterministically detect the variant
