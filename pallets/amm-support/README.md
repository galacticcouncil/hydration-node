# AMM support pallet

Support pallet for AMMs. Includes the unified event that is emitted by all AMM pallets.

TODO

we should override the deposit event

replace amm specific types i the pallet

make the types specific in amm pallet

OtcOrderId in filler etc and use u32

also for AssetFee, we need only account in as generic

dont use associated type , use tighlty coopled it

ExecutioonIdStack can be alias bounded vec

and when we push pop, use increase_stack and decrase_decrase

we ExecutionIdIdStack and ExecutionTypeStack does the same,  we dont need the exetionTypeStack trait at all

on_intialize should also directly.

we can make storagw=e non-persistent, so we dont need on initialize

IncrementalIdProvuder is not needed

otc pallet we dont need into<32>. instead use atleastu32Unsuged


remove nonfungibalbe asset type


input and output should be tpye instead of tuple, because asset fee is also

having same strucutre for inputs, outputs and fees

next inceremetnal id use overflwoing add

call it context instead of stack