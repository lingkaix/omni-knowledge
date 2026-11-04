# Engineering Principles

This document owns general design and engineering principles. It does not choose product behavior, a language, a framework, or a deployment model.

Agent-native engineering means that agents perform substantial work within human intent and authority. Design the repository for contributors who repeatedly arrive with a fresh context. Delegation remains fallible: reconsider it when repeated independent calibration cannot meet required correctness at acceptable engineering cost. Useful engineer participation is not a failure or a count to minimize.

## Derive The Solution From The Need

State the useful outcome before the method. Separate requirements from assumptions and current implementation. Test the cheapest consequential assumption before investing in a design.

Choose the smallest coherent solution that meets the complete need. A smaller diff is not better when it leaves behavior incomplete. Existing architecture can change after the engineer accepts the governing decision.

## Keep Responsibilities Clear

Give each module one clear responsibility. Keep related behavior together. Expose a small interface that hides internal decisions and ordering constraints. Minimize dependencies and avoid cycles.

Use the five SOLID principles as design questions: single responsibility, extension at real variation points, substitutable behavior, focused interfaces, and dependency direction. These principles do not require classes, interfaces, or layers without a current need.

Keep it simple (KISS). Store each decision once (DRY). Build only what is needed now (YAGNI). Similar syntax does not prove shared responsibility. Prefer composition where it keeps responsibilities clear.

Before adding a concept, ask whether an existing owner can provide the behavior. Before adding durable state, ask whether existing facts can derive it. Add a registry or extension mechanism only when actual variants justify it. For every new mechanism, layer, or registry, state its present necessity and ask what required behavior fails if it is dropped. Use an ablation, removing or disabling it under a capable observation, before claiming it is necessary or safely removable. A green suite that cannot observe its responsibility proves neither claim.

At a boundary, define inputs, outputs, invariants, errors, and side effects. Use one meaning for each term within that boundary. Translate explicitly when another boundary uses a different meaning. A deep module reduces the concepts, internal ordering, and compensation steps that callers must know. A synonym facade over the same steps does not deepen a module. Keep one semantic owner per mechanism without merging distinct storage, authentication, effect, lifecycle, or offline authorities.

## Make The System Discoverable

Use clear names, cohesive directories, local guides, and visible interfaces. A guide should lead a new contributor to the owner and its consumers. It should not copy an API catalog.

Preserve reasons that a future maintainer cannot infer from code. Keep a rule in its owner and its reason in a decision record. Link them instead of repeating them.

## Protect Data And Contracts Deliberately

State compatibility obligations in the relevant product contract. Do not assume that all interfaces require permanent compatibility or that all breaking changes are free.

Before changing retained data or authored configuration, define continuity, migration, recovery, and failure behavior. Preserve data according to the accepted contract. Never use a refactor as permission to discard it.

For extensible formats, distinguish optional additions from required semantics. Ignore unknown optional content only when the contract permits it. Reject unknown values that affect authority or required meaning.

Keep separate effect domains explicit. An effect domain is a resource or external system whose effects have a distinct owner. Do not claim atomic success across domains without a mechanism that provides it. Report unknown results honestly and define safe inspection or retry behavior.

## Keep Feedback Useful

Prefer fast, local, repeatable checks. Detect invalid assumptions at the responsible boundary and return a useful error. Fast failure does not justify blind retry.

Use code smells to locate a concrete cost or risk. Inspect change amplification, caller knowledge, duplicate decisions, and unclear ownership before proposing a refactor.

Separate essential complexity, which follows from requirements, from accidental complexity. Return the cost of a requirement to the engineer as a trade-off. Remove accidental complexity within the authorized scope.

Metrics explain observations; they are not targets for agents. File counts, coverage, review counts, and document length do not prove quality. Structural fitness numbers are signals, not line limits or permission to split a cohesive owner.

For material structural work, inventory both new concepts and parallel copies of existing concepts before and after the change. Count aliases, wrappers, registries, and duplicated interpretation rules on both sides of that inventory. A smaller export count can hide a new parallel owner; inspect responsibility and real consumers as well as the number. When export, import, or structural debt has a ratchet, allow only shrinkage against the recorded baseline. Do not raise its ceiling to fit a candidate; a genuine contract expansion needs an explicit engineer decision and explanation. A ratchet detects drift in its named inventory, not correctness or global simplicity.

## Learn Without Adding Ceremony

Treat methods as revisable. A useful pattern becomes a binding rule only when its protection justifies its cost. Automate a rule when violations are directly observable and enforcement costs less than repeated failure.

During a change, improve the affected design where a concrete need exists. Record adjacent findings without silently expanding scope. Start wider maintenance from evidence such as repeated defects or costly changes.

For removal or simplification, state the expected effect and compare before and after with the same observation. Preserve behavior with characterization tests where the current behavior lacks coverage. [Test strategy](specs/test-strategy.md) owns independent replacement and retirement proof. After correction, measure the same structural signal again; repeated recurrence is a structural question for the engineer.
