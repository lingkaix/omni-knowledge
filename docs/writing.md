# Technical Writing

This document owns the repository's STE100-based writing style. It uses selected Simplified Technical English rules, not the full controlled dictionary or a certification claim.

## Write Clear Instructions

- State the main point first.
- Use one idea per sentence and one topic per paragraph.
- Name the actor and use the active voice.
- Put a condition before its instruction.
- Use the imperative mood for procedures.
- Prefer a verb to a noun phrase for an action.
- Keep necessary articles and qualifiers.
- Use one technical term for one meaning within a context.
- Explain an abbreviation at first use.
- Use a numbered list for an ordered procedure.

Aim for about 20 words in a procedural sentence and 25 words in a descriptive sentence. These are targets. Preserve precision when a shorter sentence would lose meaning. Rationale, alternatives, trade-offs, and consequences are exempt from the two length targets, but not the other writing rules.

Avoid vague claims such as robust, seamless, or comprehensive without a concrete property. Replace “ensure” with the condition to check. Avoid noun clusters of more than three words.

Use headings that name the subject or decision. Keep each sentence and paragraph on one source line. Code examples and tables retain their required syntax.

## Organize By Concept

Open each document with its responsibility and exclusions where a reader could confuse ownership. Split by concept, not chapter length; an entry document retains shared definitions, invariants, boundaries, and child links. About 6,000 words, or inability to read the owner with its governed code, triggers a split review rather than a size gate. Another context checks that all criteria survive a split.

Write normative text in domain terms that survive a rebuild in another language or stack. Constrain public boundaries and invariants, not replaceable helper names, file layout, or line references. Put implementation navigation in a labelled projection or local guide. When technology is a governing choice, state its reason and replacement condition. Mark experience-based numbers as current defaults and state the property they protect; format-defining values remain contractual.

The [admitted rebuild corpus](documentation.md#rebuild-from-admitted-contracts) bounds what a rebuild may use. A rebuild-oriented document does not require copying implementation into the specification. A realistic user persona or ask in an acceptance scenario may use ordinary user language; surrounding engineering guidance still follows this style.

## Preserve Meaning

Keep every condition that can affect behavior, tests, failure, recovery, ownership, or responsibility. Do not shorten a rule by removing its exception or scope.

State each rule once and link to its owner. Show an example when a boundary can be misread. Separate a requirement from a current default or an observation. A substantive rewrite needs the criterion inventory and independent comparison required by [documentation governance](documentation.md). Do not accept your own rewrite as independently checked.

Use the language-standard documentation form for code. Describe purpose and constraints rather than repeating syntax. Keep terminology consistent with the owning interface or contract.

Apply this style to changed engineering text. Do not expand a task into a rewrite of unrelated product documentation.
