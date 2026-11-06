# Actual native inventory credit lifetime

The c13 opaque inventory consumer retained actual page/context holdings through their
row/source dependents, but did not finish those holdings when the last owner died.
Core intentionally makes `Holding::drop` release nothing. Real owned-page regression
on unchanged4ca production failed: 25,181,353,984 available versus 25,183,451,136
expected after the last row died — exactly the missing 2,097,152-byte page holding.
That is a real bounded-consumer defect, not physical RSS evidence or an M1-core regression.

A private drop guard now finishes only the actual known-settled synchronous inventory
holding. Replay/store/mount/policy fields precede that guard, so paid metadata and actual
FDs die first. Constructors install it before any fallible validation/inspection: refused
page/context inputs cannot lose their empty owned funding. Existing scan scratch remains
caller-owned/borrowed and must be explicitly finished after its actual report dies.
The test helpers now do that rather than incorrectly treating `drop(scratch)` as settlement.
No resource account, pool, budget counter or source authority has been added.

`NativeInventoryPage::sources` borrows the page object but yields the same opaque actual
row owners. Consuming `into_funding` returns the ONE original holding only with exclusive
ownership, after replay data and dependent inventory fields die; otherwise it returns
the owning page unchanged. A caller can iterate one paid page slot without a global
release/reserve gap or repeated claim-debit loss. No DTO can construct an observed source.

Actual owned-account controls retain credit through an escaping row and an actual native
FD/version delivery callback; dropping only the source struct is insufficient. The last
row and context return their real holdings, including refused constructors. Sixteen real
replays recover the same holding under ALL-global-credit pressure. An actual M1 UUID/source
FD/hash/binding also opens under a fork funded from its SAME real current source Claim,
then destroys its native FD checks before exact rejoin with zero free global credit.
This composes the funded SDK scope with the real M1 consumer, not a fake host/backend.

The original canonical M1 identity/rename/current-FD/intent controls remain. This repair
is not whole-corpus generation publication, query/startup activation, model quality,
physical/native/full-product acceptance, or closure of the held M6/M2 obligations.
