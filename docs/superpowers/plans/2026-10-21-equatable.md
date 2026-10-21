# Equatable generic constraints

Continue autonomous roadmap work under SPEC25/D032. All subagents use GPT 6 Luna
at Max. Keep implementation and independent expected results separately owned.

- [x] Specify the built-in equality predicate and all instantiation boundaries.
- [x] Establish independent red conformance and reproduce any existing escape.
- [x] Parse generic constraints and check universal bodies/instantiations.
- [x] Add reusable membership/deduplication library examples.
- [x] Independently review implementation, fix findings and preserve budgets.
- [x] Run full tests, mutation watchdog, formatting and denied-warning Clippy.
- [x] Record actual evidence and commit the completed slice.

Parent owns parser/docs/examples. Analyzer worker owns type constraints and
instantiation checks. Independent author owns conformance/fuzz; separate reviewer
audits escapes and final implementation. Runtime changes only if evidence shows
they are required; constraints erase after checking.
