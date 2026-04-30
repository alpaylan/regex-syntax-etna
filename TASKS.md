# regex-syntax — ETNA Tasks

Total tasks: 16

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `blank_class_5241919_1` | proptest | `BlankClassCorrect` | `witness_blank_class_correct_case_newline` |
| 002 | `blank_class_5241919_1` | quickcheck | `BlankClassCorrect` | `witness_blank_class_correct_case_newline` |
| 003 | `blank_class_5241919_1` | crabcheck | `BlankClassCorrect` | `witness_blank_class_correct_case_newline` |
| 004 | `blank_class_5241919_1` | hegel | `BlankClassCorrect` | `witness_blank_class_correct_case_newline` |
| 005 | `counted_rep_directive_7b1599f_1` | proptest | `DirectiveRepRejected` | `witness_directive_rep_rejected_case_i_one` |
| 006 | `counted_rep_directive_7b1599f_1` | quickcheck | `DirectiveRepRejected` | `witness_directive_rep_rejected_case_i_one` |
| 007 | `counted_rep_directive_7b1599f_1` | crabcheck | `DirectiveRepRejected` | `witness_directive_rep_rejected_case_i_one` |
| 008 | `counted_rep_directive_7b1599f_1` | hegel | `DirectiveRepRejected` | `witness_directive_rep_rejected_case_i_one` |
| 009 | `empty_class_print_e6d251a_1` | proptest | `PrintRoundtripParses` | `witness_print_roundtrip_parses_case_neg_any` |
| 010 | `empty_class_print_e6d251a_1` | quickcheck | `PrintRoundtripParses` | `witness_print_roundtrip_parses_case_neg_any` |
| 011 | `empty_class_print_e6d251a_1` | crabcheck | `PrintRoundtripParses` | `witness_print_roundtrip_parses_case_neg_any` |
| 012 | `empty_class_print_e6d251a_1` | hegel | `PrintRoundtripParses` | `witness_print_roundtrip_parses_case_neg_any` |
| 013 | `negation_handling_c4865a0_1` | proptest | `NamedNeqComplements` | `witness_named_neq_complements_case_space` |
| 014 | `negation_handling_c4865a0_1` | quickcheck | `NamedNeqComplements` | `witness_named_neq_complements_case_space` |
| 015 | `negation_handling_c4865a0_1` | crabcheck | `NamedNeqComplements` | `witness_named_neq_complements_case_space` |
| 016 | `negation_handling_c4865a0_1` | hegel | `NamedNeqComplements` | `witness_named_neq_complements_case_space` |

## Witness Catalog

- `witness_blank_class_correct_case_newline` — Newline (U+000A) lies inside the inverted range [0x09, 0x20] but is not blank.
- `witness_blank_class_correct_case_zero` — U+0000 — confirms control bytes do not leak into the class.
- `witness_blank_class_correct_case_uppercase_a` — U+0041 — outside the inverted range; both versions agree.
- `witness_blank_class_correct_case_space` — Positive case: ' ' must remain in the class.
- `witness_blank_class_correct_case_tab` — Positive case: '\t' must remain in the class.
- `witness_directive_rep_rejected_case_i_one` — `(?i){1}` — the original regression input.
- `witness_directive_rep_rejected_case_m_one_one` — `(?m){1,1}` — explicit-min counted form from the regression test.
- `witness_directive_rep_rejected_case_x_zero` — `(?x){0}` — zero-count edge case (also from the regression test).
- `witness_directive_rep_rejected_case_isx_five` — `(?isx){5}` — multi-flag directive followed by larger count.
- `witness_print_roundtrip_parses_case_neg_any` — `\P{any}` is the canonical empty Unicode class.
- `witness_print_roundtrip_parses_case_intersect_empty` — `[a&&b]` — intersection of two disjoint singletons.
- `witness_print_roundtrip_parses_case_subtract_self` — `[a-c--a-c]` — class minus itself, also empty.
- `witness_print_roundtrip_parses_case_simple_literal` — Sanity check: a normal pattern still roundtrips.
- `witness_named_neq_complements_case_space` — U+0020 is gc=Zs; under the bug `\p{gc!=Z}` mistakenly contains it.
- `witness_named_neq_complements_case_uppercase_a` — 'A' is gc=Lu; under the bug `\p{gc!=Z}` excludes it (it equals gc=Z).
- `witness_named_neq_complements_case_no_break_space` — U+00A0 NO-BREAK SPACE is also gc=Zs — confirms the bug isn't ASCII-specific.
