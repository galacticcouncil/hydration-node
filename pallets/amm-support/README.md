# AMM support pallet

Support pallet for AMMs. Includes the unified event that is emitted by all AMM pallets.

TODO

replace amm specific types i the pallet

make the types specific in amm pallet

OtcOrderId in filler etc and use u32


on_intialize should also directly.

we can make storagw=e non-persistent, so we dont need on initialize

otc pallet we dont need into<32>. instead use atleastu32Unsuged


remove nonfungibalbe asset type


input and output should be tpye instead of tuple, because asset fee is also

having same strucutre for inputs, outputs and fees
