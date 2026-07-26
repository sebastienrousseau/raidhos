---
name: Feature request
about: Suggest a new capability or an enhancement to an existing one.
title: ""
labels: [enhancement, triage]
assignees: []
---

<!-- SPDX-License-Identifier: GPL-3.0-only -->

## Problem

<!-- What user-facing problem does this solve? Who hits it and how
     often? -->

## Proposal

<!-- What should RaidhOS do? Be specific about the user-visible
     contract — CLI flag shape, GUI surface, error message wording. -->

## Alternatives considered

<!-- Workarounds you already use, or other designs you weighed. -->

## Trade-offs

<!-- What does this proposal cost? More flags = more surface to
     document. More dependencies = more supply-chain risk. A new
     destructive subcommand = more safety controls to author. -->

## Safety impact

- [ ] No new destructive paths.
- [ ] If destructive: opt-in flag double-gating is preserved.
- [ ] If touching privileged code: the boundary remains in
      `raidhos-priv-helper`, not in the CLI / UI.

## Related work

<!-- Links to upstream issues, prior art (Ventoy, balenaEtcher,
     Rufus), threat-model considerations. -->
