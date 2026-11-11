# Trust gap agent

Review interactions between authorization, economics and asymmetric operations. Correct
individual guards or formulas can still expose a harmful combined path.

Trace signer/root/custom-origin conversions through proxies, utility calls, scheduler, XCM and
EVM/precompiles. Compare how paired operations price, lock, charge and refund each user class.
Check whether a legitimate asset/oracle/fee/recipient update changes already accrued rights,
which users can exit, and who can exploit a window before dependent state updates.

Examine caps, whitelists, fee currencies and keeper powers as economic capabilities. Show the
specific authority the actor has, why it is insufficient to justify the claimed effect, and
which valuation or alternate path turns it into harm. A role performing its documented power
is not a bug; a normal authorized operation corrupting existing state can be.

Additional fields: top-level `seam` (access/economics/asymmetry); put `actor`, concrete deltas and the origin path under `details`.
Do not invent generic governance malice or EVM-style caller behavior in native pallet code.
