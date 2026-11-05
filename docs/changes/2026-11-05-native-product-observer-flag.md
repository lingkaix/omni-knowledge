# Native nonblocking flag in the product observer

The macOS portion of42018462814 timed out in the product-boundary test; that is not a product success or an observed B2 allocation result. Its Unix pipe observer used Linux's literal `O_NONBLOCK` on Darwin. The Darwin build now uses the existing libc dependency's native constant, so the same bounded observer does not silently leave its pipe reads blocking. Linux's existing flag and every frozen fixture, case, allocation/RSS ceiling, assertion and collection expectation remain unchanged.

The Linux product-boundary test passes on the current public implementation. Actual Darwin execution is still required. This instrument port does not erase the distinct private funded-M3 B2 allocation failure, native child uncertainty, or any product/authority/acceptance gap.
