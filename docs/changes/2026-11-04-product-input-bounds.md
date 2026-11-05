# Retained Product MCP And XLSX Bound Corrections

Status: scoped B1/B2 implementation; independent public-entrypoint resource regression and consequential review remain required.
The engineer assigned these existing-budget corrections from exact retained-product base `923fa107aae5b91d970365d7b8be6e3ed29d7847`.
[Product](../specs/product.md), [processing](../specs/processing.md#formats-and-bounds), and [agent operations](../agents.md) own the retained capabilities and bounded representations.
This record changes neither the accepted future service nor product scope.

## B1: Admit MCP Bytes Before Complete-Line Allocation

The retained stdio server previously used an unbounded line reader before its 4 MiB check.
A private byte-frame reader now checks each buffered fragment before appending and parsing.
It retains at most 4 MiB plus one possible CR delimiter byte; standard vector allocation can have bounded spare capacity.
LF and CRLF retain the existing payload ceiling; a valid final EOF frame still works and empty EOF ends the server.
The extra CR is a delimiter only when followed by LF; otherwise the existing payload bound still applies.
Invalid UTF-8 or over-limit bytes terminate the transport with fixed input-free errors, without draining an unbounded sender or dispatching a prefix.
Malformed/incomplete JSON receives the existing parse-error code with a fixed message and no reflected input.
Blank frames, valid JSON-RPC, notification handling, and the existing tool catalog remain unchanged.
This is not a new transport deadline, an output/corpus limit, a global RSS cap, or implementation of the future service's separate 1 MiB DTO contract.

## B2: Bound XLSX Text While It Is Produced

Admitted shared-string XML could previously expand into one owned string per reference, then a row join and unrestricted output.
XLSX now borrows shared text nodes from the admitted XML document and appends only a fitting UTF-8 prefix.
The same caller-selected positive representation budget applies before shared/inline/numeric text, separators, or sheet headings are copied into output.
There is no row collection, per-reference shared-string clone, full row join, or full-output staging.
Rich text runs, empty cells, tab/newline placement, natural worksheet order, and exact fitting-prefix semantics remain unchanged.
An explicit omission flag preserves public partial status and the existing warning, including a cutoff inside a multibyte character.
Later selected XML is still charged and parsed after output truncation, so later malformed XML or expanded-input exhaustion remains a failure.
Assembly-owned text is bounded by the selected output budget; shared text is borrowed from admitted input, not copied per reference.
Source/XML bytes, node/name tables, and standard string spare capacity need additional working storage.
ZIP/XML parser internal allocation remains separately unqualified; this is not an installed total-process allocation cap.
No new format ceiling, decoder, dependency, registry, job supervisor, or public test-only interface is added.

## Necessity And Reasoned Ablation

Removing B1 admission restores complete-line allocation and indefinite delimiter draining before the existing size refusal.
Truncating a read and parsing the accepted prefix could falsely dispatch an incomplete frame; refusing the transport avoids that route.
Removing B2 append admission restores repeated shared-text and joined-row expansion before the final truncation.
Borrowing avoids the amplification even for nested shared text; the omission flag is necessary because a bounded string alone cannot report whether a suffix existed.
Continuing selected-member validation preserves existing failure behavior rather than accepting a truncated prefix of a malformed archive.
No uncertain removal claim requires a stress experiment; independent resource observation remains separate from this reasoned ablation.

## Evidence And Unchanged Blockers

The assigned external m0-bound-fix log records exact base/final identities, focused commands, raw exits, synthetic fixtures, process settlement, and retained RED/setup observations.
Builder observations use the actual compiled root CLI/MCP and public extractor with real local ZIP/XML, not a fake parser/filesystem.
The earlier open-stdin over-limit RED is preserved rather than reclassified as setup failure.
No lasting test, test scope, guard, ceiling, gate, manifest/lock, vendor, Core/cache/wire, or UI is changed.
The actual retained product does not consume the vendored kernel; its kernel test allocator/RSS assertions install no product cap.
B3 corpus/query working sets, B4 generated-media and child budgets, PDF/HTML internals, ZIP central-directory allocation, full parser qualification, and native macOS proof remain open.
The supervisor's full Mac product-bound condition remains unmet; full M0 acceptance stays stopped.
No standalone Mac test split, capability reduction, push, or B3/B4 closure is included.
