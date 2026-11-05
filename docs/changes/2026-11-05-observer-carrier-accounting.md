# Local observer carrier accounting
The integrated40e8e01 Cargo tests passed, but maintained accounting treated the deliberately failing child libtest as another failed outer case; original logs remain in trunk-integration-0105.
Only the parent test is an inventoried case: suppress its child framework stdout and emit the real collected status plus escaped ownership marker as one tagged diagnostic.
Child panic stderr, actual exit/PID/marker assertions, no-late-work checks, production and all29 parent predicates remain unchanged; no gate/ignore exception.
Run the maintained workspace gate on this forward fix; child admissions remain controls rather than extra features or product failures.
This ordinary test-carrier fix requires no new independent-review or frozen OLD/NEW cycle under the01:05 direction.
