## Outcome

Describe the user-visible or operational result.

Closes:

## Scope

- Included:
- Excluded:

## Authority and privacy

- Server-authority impact:
- Hidden-information impact:
- Protocol or persistence compatibility:

## Evidence

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] Applicable property/invariant tests
- [ ] Applicable integration/reconnect/recovery tests
- [ ] Manual or visual verification when relevant

Paste concise results or explain checks that could not run.

## Risk and rollback

- Main residual risk:
- Rollback or disable path:

## Documentation

- [ ] Requirements, ADRs, agile status, and user/operator documentation are updated where needed.

## Definition of Done

- [ ] Acceptance criteria are met.
- [ ] Invalid-input paths are tested.
- [ ] No hidden cards, deck state, secrets, or reconnect tokens enter responses or ordinary logs.
- [ ] No known critical or high-severity defect remains in this slice.
