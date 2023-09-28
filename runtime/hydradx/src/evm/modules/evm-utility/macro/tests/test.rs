//                    :                     $$\   $$\                 $$\                    $$$$$$$\  $$\   $$\
//                  !YJJ^                   $$ |  $$ |                $$ |                   $$  __$$\ $$ |  $$ |
//                7B5. ~B5^                 $$ |  $$ |$$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  $$ |  $$ |\$$\ $$  |
//             .?B@G    ~@@P~               $$$$$$$$ |$$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ $$ |  $$ | \$$$$  /
//           :?#@@@Y    .&@@@P!.            $$  __$$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |$$ |  $$ | $$  $$<
//         ^?J^7P&@@!  .5@@#Y~!J!.          $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |$$ |  $$ |$$  /\$$\
//       ^JJ!.   :!J5^ ?5?^    ^?Y7.        $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |$$$$$$$  |$$ /  $$ |
//     ~PP: 7#B5!.         :?P#G: 7G?.      \__|  \__| \____$$ | \_______|\__|      \_______|\_______/ \__|  \__|
//  .!P@G    7@@@#Y^    .!P@@@#.   ~@&J:              $$\   $$ |
//  !&@@J    :&@@@@P.   !&@@@@5     #@@P.             \$$$$$$  |
//   :J##:   Y@@&P!      :JB@@&~   ?@G!                \______/
//     .?P!.?GY7:   .. .    ^?PP^:JP~
//       .7Y7.  .!YGP^ ?BP?^   ^JJ^         This file is part of https://github.com/galacticcouncil/HydraDX-node
//         .!Y7Y#@@#:   ?@@@G?JJ^           Built with <3 for decentralisation.
//            !G@@@Y    .&@@&J:
//              ^5@#.   7@#?.               Copyright (C) 2021-2023  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0

#[cfg(test)]
mod tests {
	#[test]
	fn generate_function_selector_works() {
		#[module_evm_utility_macro::generate_function_selector]
		#[derive(Debug, Eq, PartialEq)]
		#[repr(u32)]
		pub enum Action {
			Name = "name()",
			Symbol = "symbol()",
			Decimals = "decimals()",
			TotalSupply = "totalSupply()",
			BalanceOf = "balanceOf(address)",
			Transfer = "transfer(address,uint256)",
		}

		assert_eq!(Action::Name as u32, 0x06fdde03_u32);
		assert_eq!(Action::Symbol as u32, 0x95d89b41_u32);
		assert_eq!(Action::Decimals as u32, 0x313ce567_u32);
		assert_eq!(Action::TotalSupply as u32, 0x18160ddd_u32);
		assert_eq!(Action::BalanceOf as u32, 0x70a08231_u32);
		assert_eq!(Action::Transfer as u32, 0xa9059cbb_u32);
	}

	#[test]
	fn keccak256_works() {
		assert_eq!(
			module_evm_utility_macro::keccak256!(""),
			&module_evm_utility::sha3_256("")
		);
		assert_eq!(
			module_evm_utility_macro::keccak256!("keccak256"),
			&module_evm_utility::sha3_256("keccak256")
		);
	}
}
