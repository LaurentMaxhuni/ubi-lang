# Equatable generic constraints

Continue autonomous roadmap work under SPEC25/D032. All subagents use GPT 6 Luna
at Max. Keep implementation and independent expected results separately owned.

- [x] Specify the built-in equality predicate and all instantiation boundaries.
- [ ] Establish independent red conformance and reproduce any existing escape.
- [ ] Parse generic constraints and check universal bodies/instantiations.
- [ ] Add reusable membership/deduplication library examples.
- [ ] Independently review implementation, fix findings and preserve budgets.
- [ ] Run full tests, mutation watchdog, formatting and denied-warning Clippy.
- [ ] Record actual evidence and commit the completed slice.

Parent owns parser/docs/examples. Analyzer worker owns type constraints and
instantiation checks. Independent author owns conformance/fuzz; separate reviewer
audits escapes and final implementation. Runtime changes only if evidence shows
they are required; constraints erase after checking.
