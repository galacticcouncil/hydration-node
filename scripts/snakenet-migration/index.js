const {ApiPromise, WsProvider, Keyring} = require("@polkadot/api");
const {Metadata, TypeRegistry} = require("@polkadot/types");
const { xxhashAsHex } = require("@polkadot/util-crypto");
const {encodeAddress} = require("@polkadot/util-crypto");
const { stringToU8a } = require("@polkadot/util");
const {compactStripLength, hexToU8a, u8aToHex}  = require('@polkadot/util');


const chalk = require("chalk");
const path  = require("path");
const fs = require("fs");
const BN = require("bn.js")
const assert = require("assert");
const moment = require("moment");

const yargs = require('yargs');
const { hideBin } = require('yargs/helpers');

const ACCOUNT_SECRET = process.env.ACCOUNT_SECRET || "//Alice";

const SOURCE_RPC = process.env.SOURCE_RPC_SERVER || "wss://rpc-02.snakenet.hydradx.io";
const TARGET_RPC = process.env.TARGET_RPC_SERVER || "ws://127.0.0.1:9988";

const storagePath = path.join(__dirname, "data", "storage.json");
const tempStoragePath = path.join(__dirname, "data", "tempStorage.json");
const finalStoragePath = path.join(__dirname, "data", "finalStorage.json");

const includedModules = [
    "System.Account",
    "Claims.Claims",
    "Tokens.TotalIssuance",
    "Tokens.Locks",
    "Tokens.Accounts",
    "Balances.TotalIssuance",
    "Balances.Account",
    "Balances.Locks",
    "Balances.Reserves",
    "Balances.StorageVersion",

    "Elections.Members",
    "Elections.RunnersUp",
    "Elections.Candidates",
    "Elections.ElectionRounds",
    "Elections.Voting",
    "Council.Proposals",
    "Council.ProposalOf",
    "Council.Voting",
    "Council.ProposalCount",
    "Council.Members",
    "Council.Prime",
    "TechnicalCommittee.Proposals",
    "TechnicalCommittee.ProposalOf",
    "TechnicalCommittee.Voting",
    "TechnicalCommittee.ProposalCount",
    "TechnicalCommittee.Members",
    "TechnicalCommittee.Prime",

    "Identity.IdentityOf",
    "Identity.SuperOf",
    "Identity.SubsOf",
    "Identity.Registrars",

    "Tips.Tips",
    "Tips.Reasons",
]

const all_modules = [
    ["System.Account", "0x26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da9"],
    ["System.ExtrinsicCount", "0x26aa394eea5630e07c48ae0c9558cef7bdc0bd303e9855813aa8a30d4efc5112"],
    ["System.BlockWeight", "0x26aa394eea5630e07c48ae0c9558cef734abf5cb34d6244378cddbf18e849d96"],
    ["System.AllExtrinsicsLen", "0x26aa394eea5630e07c48ae0c9558cef7a86da5a932684f199539836fcb8c886f"],
    ["System.BlockHash", "0x26aa394eea5630e07c48ae0c9558cef7a44704b568d21667356a5a050c118746"],
    ["System.ExtrinsicData", "0x26aa394eea5630e07c48ae0c9558cef7df1daeb8986837f21cc5d17596bb78d1"],
    ["System.Number", "0x26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac"],
    ["System.ParentHash", "0x26aa394eea5630e07c48ae0c9558cef78a42f33323cb5ced3b44dd825fda9fcc"],
    ["System.Digest", "0x26aa394eea5630e07c48ae0c9558cef799e7f93fc6a98f0874fd057f111c4d2d"],
    ["System.Events", "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7"],
    ["System.EventCount", "0x26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850"],
    ["System.EventTopics", "0x26aa394eea5630e07c48ae0c9558cef7bb94e1c21adab714983cf06622e1de76"],
    ["System.LastRuntimeUpgrade", "0x26aa394eea5630e07c48ae0c9558cef7f9cce9c888469bb1a0dceaa129672ef8"],
    ["System.UpgradedToU32RefCount", "0x26aa394eea5630e07c48ae0c9558cef75684a022a34dd8bfa2baaf44f172b710"],
    ["System.UpgradedToTripleRefCount", "0x26aa394eea5630e07c48ae0c9558cef7a7fd6c28836b9a28522dc924110cf439"],
    ["System.ExecutionPhase", "0x26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a"],
    ["RandomnessCollectiveFlip.RandomMaterial", "0xbd2a529379475088d3e29a918cd478721a39ec767bd5269111e6492a1675702a"],
    ["Babe.EpochIndex", "0x1cb6f36e027abb2091cfb5110ab5087f38316cbf8fa0da822a20ac1c55bf1be3"],
    ["Babe.Authorities", "0x1cb6f36e027abb2091cfb5110ab5087f5e0621c4869aa60c02be9adcc98a0d1d"],
    ["Babe.GenesisSlot", "0x1cb6f36e027abb2091cfb5110ab5087f678711d15ebbceba5cd0cea158e6675a"],
    ["Babe.CurrentSlot", "0x1cb6f36e027abb2091cfb5110ab5087f06155b3cd9a8c9e5e9a23fd5dc13a5ed"],
    ["Babe.Randomness", "0x1cb6f36e027abb2091cfb5110ab5087f7a414cb008e0e61e46722aa60abdd672"],
    ["Babe.PendingEpochConfigChange", "0x1cb6f36e027abb2091cfb5110ab5087f8b4328e343c3e0ac90f83da4860cbe36"],
    ["Babe.NextRandomness", "0x1cb6f36e027abb2091cfb5110ab5087f7ce678799d3eff024253b90e84927cc6"],
    ["Babe.NextAuthorities", "0x1cb6f36e027abb2091cfb5110ab5087faacf00b9b41fda7a9268821c2a2b3e4c"],
    ["Babe.SegmentIndex", "0x1cb6f36e027abb2091cfb5110ab5087f66e8f035c8adbe7f1547b43c51e6f8a4"],
    ["Babe.UnderConstruction", "0x1cb6f36e027abb2091cfb5110ab5087fb9093659d7a856809757134d2bc86e62"],
    ["Babe.Initialized", "0x1cb6f36e027abb2091cfb5110ab5087ffa92de910a7ce2bd58e99729c69727c1"],
    ["Babe.AuthorVrfRandomness", "0x1cb6f36e027abb2091cfb5110ab5087fd077dfdb8adb10f78f10a5df8742c545"],
    ["Babe.EpochStart", "0x1cb6f36e027abb2091cfb5110ab5087fe90e2fbf2d792cb324bffa9427fe1f0e"],
    ["Babe.Lateness", "0x1cb6f36e027abb2091cfb5110ab5087f0323475657e0890fbdbf66fb24b4649e"],
    ["Babe.EpochConfig", "0x1cb6f36e027abb2091cfb5110ab5087fdc6b171b77304263c292cc3ea5ed31ef"],
    ["Babe.NextEpochConfig", "0x1cb6f36e027abb2091cfb5110ab5087f9aab0a5b63b359512deee557c9f4cf63"],
    ["Timestamp.Now", "0xf0c365c3cf59d671eb72da0e7a4113c49f1f0515f462cdcf84e0f1d6045dfcbb"],
    ["Timestamp.DidUpdate", "0xf0c365c3cf59d671eb72da0e7a4113c4bbd108c4899964f707fdaffb82636065"],
    ["Grandpa.State", "0x5f9cc45b7a00c5899361e1c6099678dcf39a107f2d8d3854c9aba9b021f43d9c"],
    ["Grandpa.PendingChange", "0x5f9cc45b7a00c5899361e1c6099678dc2ff65991b1c915dd6cc8d4825eacfcb4"],
    ["Grandpa.NextForced", "0x5f9cc45b7a00c5899361e1c6099678dc01d7818126bd9b3074803e91f4c91b59"],
    ["Grandpa.Stalled", "0x5f9cc45b7a00c5899361e1c6099678dc7ddd013461b72c3004f9c0ca3faf9ebe"],
    ["Grandpa.CurrentSetId", "0x5f9cc45b7a00c5899361e1c6099678dc8a2d09463effcc78a22d75b9cb87dffc"],
    ["Grandpa.SetIdSession", "0x5f9cc45b7a00c5899361e1c6099678dcd47cb8f5328af743ddfb361e7180e7fc"],
    ["Balances.TotalIssuance", "0xc2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80"],
    ["Balances.Account", "0xc2261276cc9d1f8598ea4b6a74b15c2fb99d880ec681799c0cf30e8886371da9"],
    ["Balances.Locks", "0xc2261276cc9d1f8598ea4b6a74b15c2f218f26c73add634897550b4003b26bc6"],
    ["Balances.Reserves", "0xc2261276cc9d1f8598ea4b6a74b15c2f60c9ab7384f36f3de79a685fa22b4491"],
    ["Balances.StorageVersion", "0xc2261276cc9d1f8598ea4b6a74b15c2f308ce9615de0775a82f8a94dc3d285a1"],
    ["TransactionPayment.NextFeeMultiplier", "0x3f1467a096bcd71a5b6a0c8155e208103f2edf3bdf381debe331ab7446addfdc"],
    ["TransactionPayment.StorageVersion", "0x3f1467a096bcd71a5b6a0c8155e20810308ce9615de0775a82f8a94dc3d285a1"],
    ["Sudo.Key", "0x5c0d1176a568c1f92944340dbfed9e9c530ebca703c85910e7164cb7d1c9e47b"],
    ["Scheduler.Agenda", "0x3db7a24cfdc9de785974746c14a99df91643f5419718219c95679ddd2d825574"],
    ["Scheduler.Lookup", "0x3db7a24cfdc9de785974746c14a99df9891ad457bf4da54990fa84a2acb148a2"],
    ["Identity.IdentityOf", "0x2aeddc77fe58c98d50bd37f1b90840f9cd7f37317cd20b61e9bd46fab8704714"],
    ["Identity.SuperOf", "0x2aeddc77fe58c98d50bd37f1b90840f943a953ac082e08b6527ce262dbd4abf2"],
    ["Identity.SubsOf", "0x2aeddc77fe58c98d50bd37f1b90840f96ee5a0b09e7e9a96219dd66f0f74c37e"],
    ["Identity.Registrars", "0x2aeddc77fe58c98d50bd37f1b90840f91f7f3f3eb1c2a69978da998d19f74ec5"],
    ["Preimage.StatusFor", "0xd8f314b7f4e6b095f0f8ee4656a4482555b1ae8eced5522f3c4049bc84eda4a8"],
    ["Preimage.PreimageFor", "0xd8f314b7f4e6b095f0f8ee4656a448257c7dda85c9c297999fd02215e8c8f9de"],
    ["Authorship.Uncles", "0xd57bce545fb382c34570e5dfbf338f5ea36180b5cfb9f6541f8849df92a6ec93"],
    ["Authorship.Author", "0xd57bce545fb382c34570e5dfbf338f5e326d21bc67a4b34023d577585d72bfd7"],
    ["Authorship.DidSetUncles", "0xd57bce545fb382c34570e5dfbf338f5ebddf84c5eb23e6f53af725880d8ffe90"],
    ["Staking.HistoryDepth", "0x5f3e4907f716ac89b6347d15ececedcaac0a2cbf8e355f5ea6cb2de8727bfb0c"],
    ["Staking.ValidatorCount", "0x5f3e4907f716ac89b6347d15ececedca138e71612491192d68deab7e6f563fe1"],
    ["Staking.MinimumValidatorCount", "0x5f3e4907f716ac89b6347d15ececedcab49a2738eeb30896aacb8b3fb46471bd"],
    ["Staking.Invulnerables", "0x5f3e4907f716ac89b6347d15ececedca5579297f4dfb9609e7e4c2ebab9ce40a"],
    ["Staking.Bonded", "0x5f3e4907f716ac89b6347d15ececedca3ed14b45ed20d054f05e37e2542cfe70"],
    ["Staking.MinNominatorBond", "0x5f3e4907f716ac89b6347d15ececedcaed441ceb81326c56263efbb60c95c2e4"],
    ["Staking.MinValidatorBond", "0x5f3e4907f716ac89b6347d15ececedca666fdcbb473985b3ac933d13f4acff8d"],
    ["Staking.MinCommission", "0x5f3e4907f716ac89b6347d15ececedca58b0c9c1fa6cc13759ead9b42db9eebe"],
    ["Staking.Ledger", "0x5f3e4907f716ac89b6347d15ececedca422adb579f1dbf4f3886c5cfa3bb8cc4"],
    ["Staking.Payee", "0x5f3e4907f716ac89b6347d15ececedca9220e172bed316605f73f1ff7b4ade98"],
    ["Staking.Validators", "0x5f3e4907f716ac89b6347d15ececedca88dcde934c658227ee1dfafcd6e16903"],
    ["Staking.CounterForValidators", "0x5f3e4907f716ac89b6347d15ececedca6ddc7809c6da9bb6093ee22e0fda4ba8"],
    ["Staking.MaxValidatorsCount", "0x5f3e4907f716ac89b6347d15ececedca98c2640cda6c0d801194a8a61c699224"],
    ["Staking.Nominators", "0x5f3e4907f716ac89b6347d15ececedca9c6a637f62ae2af1c7e31eed7e96be04"],
    ["Staking.CounterForNominators", "0x5f3e4907f716ac89b6347d15ececedcaf99b25852d3d69419882da651375cdb3"],
    ["Staking.MaxNominatorsCount", "0x5f3e4907f716ac89b6347d15ececedcad642c00af119adf30dc11d32e9f0886d"],
    ["Staking.CurrentEra", "0x5f3e4907f716ac89b6347d15ececedca0b6a45321efae92aea15e0740ec7afe7"],
    ["Staking.ActiveEra", "0x5f3e4907f716ac89b6347d15ececedca487df464e44a534ba6b0cbb32407b587"],
    ["Staking.ErasStartSessionIndex", "0x5f3e4907f716ac89b6347d15ececedcaad811cd65a470ddc5f1d628ff0550982"],
    ["Staking.ErasStakers", "0x5f3e4907f716ac89b6347d15ececedca8bde0a0ea8864605e3b68ed9cb2da01b"],
    ["Staking.ErasStakersClipped", "0x5f3e4907f716ac89b6347d15ececedca42982b9d6c7acc99faa9094c912372c2"],
    ["Staking.ErasValidatorPrefs", "0x5f3e4907f716ac89b6347d15ececedca682db92dde20a10d96d00ff0e9e221c0"],
    ["Staking.ErasValidatorReward", "0x5f3e4907f716ac89b6347d15ececedca7e6ed2ee507c7b4441d59e4ded44b8a2"],
    ["Staking.ErasRewardPoints", "0x5f3e4907f716ac89b6347d15ececedca80cc6574281671b299c1727d7ac68cab"],
    ["Staking.ErasTotalStake", "0x5f3e4907f716ac89b6347d15ececedcaa141c4fe67c2d11f4a10c6aca7a79a04"],
    ["Staking.ForceEra", "0x5f3e4907f716ac89b6347d15ececedcaf7dad0317324aecae8744b87fc95f2f3"],
    ["Staking.SlashRewardFraction", "0x5f3e4907f716ac89b6347d15ececedcac29a0310e1bb45d20cace77ccb62c97d"],
    ["Staking.CanceledSlashPayout", "0x5f3e4907f716ac89b6347d15ececedca28dccb559b95c40168a1b2696581b5a7"],
    ["Staking.UnappliedSlashes", "0x5f3e4907f716ac89b6347d15ececedca042824170a5db4381fe3395039cabd24"],
    ["Staking.BondedEras", "0x5f3e4907f716ac89b6347d15ececedcaea07de2b8f010516dca3f7ef52f7ac5a"],
    ["Staking.ValidatorSlashInEra", "0x5f3e4907f716ac89b6347d15ececedcaad6e15ee7bfd5d55eba1012487d3af54"],
    ["Staking.NominatorSlashInEra", "0x5f3e4907f716ac89b6347d15ececedca815008e8210b6d6cf701e22e5bf27141"],
    ["Staking.SlashingSpans", "0x5f3e4907f716ac89b6347d15ececedcaab6a212bc08a5603828f33f90ec4a139"],
    ["Staking.SpanSlash", "0x5f3e4907f716ac89b6347d15ececedcae62f6f797ebe9138dfced942977fea50"],
    ["Staking.EarliestUnappliedSlash", "0x5f3e4907f716ac89b6347d15ececedca605b2c046b5509037f3f158b9741d037"],
    ["Staking.CurrentPlannedSession", "0x5f3e4907f716ac89b6347d15ececedcac0d39ff577af2cc6b67ac3641fa9c4e7"],
    ["Staking.OffendingValidators", "0x5f3e4907f716ac89b6347d15ececedcaa2721b5fdc019ff2482f9172ab882a78"],
    ["Staking.StorageVersion", "0x5f3e4907f716ac89b6347d15ececedca308ce9615de0775a82f8a94dc3d285a1"],
    ["Staking.ChillThreshold", "0x5f3e4907f716ac89b6347d15ececedcacddc49c5f30807d474a09d70fed8a569"],
    ["Democracy.PublicPropCount", "0xf2794c22e353e9a839f12faab03a911bbdcb0c5143a8617ed38ae3810dd45bc6"],
    ["Democracy.PublicProps", "0xf2794c22e353e9a839f12faab03a911b49d40ca9ee2e46158745d0ab5442ac80"],
    ["Democracy.DepositOf", "0xf2794c22e353e9a839f12faab03a911b255521173d2e7e678ffbf1e6bb8a6257"],
    ["Democracy.Preimages", "0xf2794c22e353e9a839f12faab03a911bf68967d635641a7087e53f2bff1ecad3"],
    ["Democracy.ReferendumCount", "0xf2794c22e353e9a839f12faab03a911b7f17cdfbfa73331856cca0acddd7842e"],
    ["Democracy.LowestUnbaked", "0xf2794c22e353e9a839f12faab03a911be2f6cb0456905c189bcb0458f9440f13"],
    ["Democracy.ReferendumInfoOf", "0xf2794c22e353e9a839f12faab03a911bb9e0c7dac4238b700a83735192cb921c"],
    ["Democracy.VotingOf", "0xf2794c22e353e9a839f12faab03a911be470c6afbbbc027eb288ade7595953c2"],
    ["Democracy.Locks", "0xf2794c22e353e9a839f12faab03a911b218f26c73add634897550b4003b26bc6"],
    ["Democracy.LastTabledWasExternal", "0xf2794c22e353e9a839f12faab03a911bfe9f3e7f80c2c73ce03922baf72a23fd"],
    ["Democracy.NextExternal", "0xf2794c22e353e9a839f12faab03a911b0ef76b8bae2d5abecdf27038f43d62d9"],
    ["Democracy.Blacklist", "0xf2794c22e353e9a839f12faab03a911bb7612c99e31defd01cd5a28e9967e208"],
    ["Democracy.Cancellations", "0xf2794c22e353e9a839f12faab03a911be6e976fedc31c7b8cf73483554bd2be2"],
    ["Democracy.StorageVersion", "0xf2794c22e353e9a839f12faab03a911b308ce9615de0775a82f8a94dc3d285a1"],
    ["ElectionProviderMultiPhase.Round", "0xede8e4fdc3c8b556f0ce2f77fc2575e313792e785168f725b60e2969c7fc2552"],
    ["ElectionProviderMultiPhase.CurrentPhase", "0xede8e4fdc3c8b556f0ce2f77fc2575e3d9764401941df7f707a47ba7db64a6ea"],
    ["ElectionProviderMultiPhase.QueuedSolution", "0xede8e4fdc3c8b556f0ce2f77fc2575e3480ca1a34cacdb12affc67ecc3a08004"],
    ["ElectionProviderMultiPhase.Snapshot", "0xede8e4fdc3c8b556f0ce2f77fc2575e396d38fd45bc038faa9586fa93aa03ef7"],
    ["ElectionProviderMultiPhase.DesiredTargets", "0xede8e4fdc3c8b556f0ce2f77fc2575e3720b70fd47fbed875a3a2dad4378ec4d"],
    ["ElectionProviderMultiPhase.SnapshotMetadata", "0xede8e4fdc3c8b556f0ce2f77fc2575e32a3e5f1d461bf763a76013e062b46c0e"],
    ["ElectionProviderMultiPhase.SignedSubmissionNextIndex", "0xede8e4fdc3c8b556f0ce2f77fc2575e314ce3a66063250f71e0a11a517ea4062"],
    ["ElectionProviderMultiPhase.SignedSubmissionIndices", "0xede8e4fdc3c8b556f0ce2f77fc2575e30bb1d35a8b4d31acec2fccbcf0172fc4"],
    ["ElectionProviderMultiPhase.SignedSubmissionsMap", "0xede8e4fdc3c8b556f0ce2f77fc2575e3b3d1c643c0d45e2bb9269ac86c1dcda0"],
    ["ElectionProviderMultiPhase.MinimumUntrustedScore", "0xede8e4fdc3c8b556f0ce2f77fc2575e36d8bbda3bbbad46cc57d43825dd040c5"],
    ["Treasury.ProposalCount", "0x89d139e01a5eb2256f222e5fc5dbe6b36254e9d55588784fa2a62b726696e2b1"],
    ["Treasury.Proposals", "0x89d139e01a5eb2256f222e5fc5dbe6b388c2f7188c6fdd1dffae2fa0d171f440"],
    ["Treasury.Approvals", "0x89d139e01a5eb2256f222e5fc5dbe6b33c9c1284130706f5aea0c8b3d4c54d89"],
    ["Session.Validators", "0xcec5070d609dd3497f72bde07fc96ba088dcde934c658227ee1dfafcd6e16903"],
    ["Session.CurrentIndex", "0xcec5070d609dd3497f72bde07fc96ba072763800a36a99fdfc7c10f6415f6ee6"],
    ["Session.QueuedChanged", "0xcec5070d609dd3497f72bde07fc96ba09450bfa4b96a3fa7a3c8f40da6bf32e1"],
    ["Session.QueuedKeys", "0xcec5070d609dd3497f72bde07fc96ba0e0cdd062e6eaf24295ad4ccfc41d4609"],
    ["Session.DisabledValidators", "0xcec5070d609dd3497f72bde07fc96ba05a9a74be4a5a7df60b01a6c0326c5e20"],
    ["Session.NextKeys", "0xcec5070d609dd3497f72bde07fc96ba04c014e6bf8b8c2c011e7290b85696bb3"],
    ["Session.KeyOwner", "0xcec5070d609dd3497f72bde07fc96ba0726380404683fc89e8233450c8aa1950"],
    ["Elections.Members", "0x7cda3cfa86b349fdafce4979b197118fba7fb8745735dc3be2a2c61a72c39e78"],
    ["Elections.RunnersUp", "0x7cda3cfa86b349fdafce4979b197118f40982df579bdf1315224f41e5f482063"],
    ["Elections.Candidates", "0x7cda3cfa86b349fdafce4979b197118f948ece45793d7f15c9c0b9574ddbc665"],
    ["Elections.ElectionRounds", "0x7cda3cfa86b349fdafce4979b197118f7657ad2ff3a6742e1071bbb898ce5431"],
    ["Elections.Voting", "0x7cda3cfa86b349fdafce4979b197118f71cd3068e6118bfb392b798317f63a89"],
    ["Council.Proposals", "0xaebd463ed9925c488c112434d61debc088c2f7188c6fdd1dffae2fa0d171f440"],
    ["Council.ProposalOf", "0xaebd463ed9925c488c112434d61debc0e9d6db8868a37d79930bc3f7f33950d1"],
    ["Council.Voting", "0xaebd463ed9925c488c112434d61debc071cd3068e6118bfb392b798317f63a89"],
    ["Council.ProposalCount", "0xaebd463ed9925c488c112434d61debc06254e9d55588784fa2a62b726696e2b1"],
    ["Council.Members", "0xaebd463ed9925c488c112434d61debc0ba7fb8745735dc3be2a2c61a72c39e78"],
    ["Council.Prime", "0xaebd463ed9925c488c112434d61debc0cb3136ee16886ac28a54f39e605b387a"],
    ["TechnicalCommittee.Proposals", "0xed25f63942de25ac5253ba64b5eb64d188c2f7188c6fdd1dffae2fa0d171f440"],
    ["TechnicalCommittee.ProposalOf", "0xed25f63942de25ac5253ba64b5eb64d1e9d6db8868a37d79930bc3f7f33950d1"],
    ["TechnicalCommittee.Voting", "0xed25f63942de25ac5253ba64b5eb64d171cd3068e6118bfb392b798317f63a89"],
    ["TechnicalCommittee.ProposalCount", "0xed25f63942de25ac5253ba64b5eb64d16254e9d55588784fa2a62b726696e2b1"],
    ["TechnicalCommittee.Members", "0xed25f63942de25ac5253ba64b5eb64d1ba7fb8745735dc3be2a2c61a72c39e78"],
    ["TechnicalCommittee.Prime", "0xed25f63942de25ac5253ba64b5eb64d1cb3136ee16886ac28a54f39e605b387a"],
    ["ImOnline.HeartbeatAfter", "0x2b06af9719ac64d755623cda8ddd9b948aa1f2c9844f11024c1d204e705a6217"],
    ["ImOnline.Keys", "0x2b06af9719ac64d755623cda8ddd9b949f99a2ce711f3a31b2fc05604c93f179"],
    ["ImOnline.ReceivedHeartbeats", "0x2b06af9719ac64d755623cda8ddd9b94cc5a1aa6e3716372f36ef103b7e3ae67"],
    ["ImOnline.AuthoredBlocks", "0x2b06af9719ac64d755623cda8ddd9b94b1c371ded9e9c565e89ba783c4d5f5f9"],
    ["Offences.Reports", "0xd5c41b52a371aa36c9254ce34324f2a5b262e9238fa402540c250bc3f5d6188d"],
    ["Offences.ConcurrentReportsIndex", "0xd5c41b52a371aa36c9254ce34324f2a560dc8ef000cdbdc859dd352229ce16fb"],
    ["Offences.ReportsByKindIndex", "0xd5c41b52a371aa36c9254ce34324f2a53589c0dac50da6fb3a3611eb32bcd27e"],
    ["Tips.Tips", "0x2c5de123c468aef7f3ac2ab3a76f87ce2c5de123c468aef7f3ac2ab3a76f87ce"],
    ["Tips.Reasons", "0x2c5de123c468aef7f3ac2ab3a76f87ced834d1db4313872258a93b9fc45d488b"],
    ["Tokens.TotalIssuance", "0x99971b5749ac43e0235e41b0d378691857c875e4cff74148e4628f264b974c80"],
    ["Tokens.Locks", "0x99971b5749ac43e0235e41b0d3786918218f26c73add634897550b4003b26bc6"],
    ["Tokens.Accounts", "0x99971b5749ac43e0235e41b0d37869188ee7418a6531173d60d1f6a82d8f4d51"],
    ["Vesting.VestingSchedules", "0x5f27b51b5ec208ee9cb25b55d87282439c806850c4ee3bc06ba62b096318fe38"],
    ["Claims.Claims", "0x9c5d795d0297be56027a4b2464e333979c5d795d0297be56027a4b2464e33397"],
    ["Faucet.Minted", "0x152b3490153351f705ff958e67c0f511797034e4c0b24d1799a1a70b9815bea5"],
    ["Faucet.MintLimit", "0x152b3490153351f705ff958e67c0f511cc4382ce9091d3d6df268eb1b6a00d69"],
    ["Faucet.Rampage", "0x152b3490153351f705ff958e67c0f511fc7a0140d9dd508721ba3d4e433060f5"],
    ["Faucet.MintableCurrencies", "0x152b3490153351f705ff958e67c0f51152b217b6e4e1e203f4f57c80d248bf7b"],
    ["MultiTransactionPayment.AccountCurrencyMap", "0x4b734cb04aff95f79e170c2aa70e635237b0b76fe9d5880c323eaa5103b09e18"],
    ["MultiTransactionPayment.AcceptedCurrencies", "0x4b734cb04aff95f79e170c2aa70e6352b5227fedb576995a45ba117fb9b6ae8d"],
    ["MultiTransactionPayment.AcceptedCurrencyPrice", "0x4b734cb04aff95f79e170c2aa70e63520af19738e7d6f052acd7c6f9ec84059b"],
    ["MultiTransactionPayment.FallbackAccount", "0x4b734cb04aff95f79e170c2aa70e63522eff9a2db148b7e07506f0461bb544f8"],
    ["GenesisHistory.PreviousChain", "0x1754677a24055221d22db56f83f5e21390895d6c6b21a85c004b8942c3bc35ae"],
]

const excludeFromTripling = [
    "7HqdGVRB4MXz1osLR77mfWoo536cWasTYsuAbVuicHdiKQXf", // Galactic council
    "7NPoMQbiA6trJKkjB35uk96MeJD4PGWkLQLH7k7hXEkZpiba", // Alice.
    "7L53bUTBopuwFt3mKUfmkzgGLayYa1Yvn1hAg9v5UMrQzTfh", // Treasury
];

const ignoreInValidate = [
    "7NPoMQbiA6trJKkjB35uk96MeJD4PGWkLQLH7k7hXEkZpiba", // Alice.
]

const log = (msg) => {
    let m = moment().format('YYYY-MM-DD HH:mm:ss') ;
    console.log(`${m} snakenet-migration \t${msg}`);
}


const hdxAddressFromKey = (key, prefix) => {
    const registry = new TypeRegistry();
    const account  = registry.createType("AccountId32", `0x${key.substring(prefix.length+32)}`);
    return encodeAddress(account, 63);
}

let modulePrefixes = new Map();

const prefixes = async (url) => {

    const api = await createClient(url);
    const metadata = await api.rpc.state.getMetadata();
    // Populate the prefixes array
    const modules = JSON.parse(JSON.stringify(metadata.asLatest.pallets));
    modules.forEach((module) => {
        //console.log("MODULES->", module);
        if (module.storage) {
            module.storage.items.forEach( (item) => {
                const storageModule = `${module.storage.prefix.toString()}.${item.name}`;
                if (includedModules.includes((storageModule))){
                    const modPrefix = xxhashAsHex(module.storage.prefix, 128);
                    const itemPrefix = xxhashAsHex(item.name, 128);
                    const storagePrefix = modPrefix.concat(itemPrefix.substring(2));
                    modulePrefixes.set(storageModule, storagePrefix);
                }
            })
        }
  });
}

const downloadData = async (url, destination, block_number = undefined) => {
    if (fs.existsSync(destination)) {
        log(
            chalk.yellow(
                "Data already downloaded. Delete ./data/storage.json and rerun the script if you want to fetch latest storage"
            )
        );
    } else {
        console.log(
            chalk.yellow(
                "Downloading ... "
            )
        );
        const stream = fs.createWriteStream(destination, {flags: "a"});
        const api = await createClient(url);

        let block_hash;

        if (block_number) {
            log(chalk.green(`Block number: ${block_number}`))
            block_hash = await api.rpc.chain.getBlockHash(block_number);
        }else{
            log(chalk.green(`Block number: latest`))
            block_hash = await api.rpc.chain.getFinalizedHead();
        }

        log(`Block hash: ${block_hash}`)

        let allPairs = [];

        for (const [key,value] of modulePrefixes) {
            const pairs = await api.rpc.state.getPairs(value);
            let p = pairs.map((elem) => JSON.parse(JSON.stringify(elem)));
            if (p.length > 0) {
                log(`Downloading ${key} - found ${chalk.yellow(p.length)}`);
                allPairs.push(...p);
            }
        }
        stream.write(JSON.stringify(allPairs));
        stream.end();
        log(destination)
    }
}

const createClient = async (rpc) => {
    log(chalk.green(`Connecting to ${rpc}`));
    return ApiPromise.create({
        provider: new WsProvider(rpc),
    });
};

const asSingleUpdates = (api, pairs) => {
    return pairs.map((keyValue) => {
            return api.tx.system.setStorage([keyValue]);
        }
    );
}

const asSudo = (api, call) => {
    return api.tx.sudo.sudo(call);
}

const chunkify = (a, size) => Array(Math.ceil(a.length / size))
  .fill()
  .map((_, i) => a.slice(i * size, i * size + size));

const sendAndWaitFinalization = ({from, tx, printEvents = []}) => new Promise(resolve =>
    tx.signAndSend(from, (receipt) => {
        let {status, events = []} = receipt;
        if (status.isInBlock) {
            log(`Included in ${status.asInBlock.toHex()}`);
            events.filter(({event: {section}}) => printEvents.includes(section))
                .forEach(({ event: { data, method, section } }) =>
                    log(`${section}.${method} ${JSON.stringify(data)}`));
        }
        if (status.isFinalized) {
            log(`Finalized ${status.asFinalized.toHex()}`);
            resolve(receipt);
        }

        log(status)
    }));

const validate = async (source_url, target_url) => {
    log("Validating triple balances")
    const api = await createClient(source_url);
    const target_api = await createClient(target_url);

    const assertBalances = async (address, balance) => {
        const account = await target_api.query.system.account(address);
        const bal = new BN(account.data.free.toString());
        const reserved = new BN(account.data.reserved.toString());
        const actual = bal.add(reserved);

        let tripled;
        if ( ! excludeFromTripling.includes(address)){
            tripled = balance.imuln(3);
        }else{
            tripled = balance;
        }
        assert( actual.eq(tripled), `Incorrect amount for ${address} - actual ${actual} expected ${tripled}`);
    }

    let balances = [];

    await api.query.system.account.entries().then( accounts => {
        accounts.map( ([key, {data}]) => {
            const [address] = key.toHuman()
            const free = new BN(data.free);
            const reserved = new BN(data.reserved);

            const balance = free.add(reserved);

            balances.push({address, balance});
        });
    })

    for (let idx in balances){
        log(`Checking ${balances[idx].address}`)
        if (!ignoreInValidate.includes(balances[idx].address)){
            await assertBalances(balances[idx].address, balances[idx].balance);
        }
    }
    log("Validating triple claims...")

    const source_claims = await api.query.claims.claims.entries()
    const source_unclaimed_count = source_claims.filter((b)=>b[1] !== 0).length
    let source_unclaimed = source_claims.filter((b)=>b[1] !== 0).map( (val) => new BN(val[1])).reduce( (a,b) => a.add(b))

    const target_claims = await target_api.query.claims.claims.entries()
    const target_unclaimed_count = target_claims.filter((b)=>b[1] !== 0).length
    const target_unclaimed = target_claims.filter((b)=>b[1] !== 0).map( (val) => new BN(val[1])).reduce( (a,b) => a.add(b))

    source_unclaimed = new BN(source_unclaimed).imuln(3);

    assert( source_unclaimed_count ===  target_unclaimed_count, "Unclaimed count does not match")
    assert( source_unclaimed.eq(target_unclaimed), "Unclaimed balance does not match")

    log(chalk.green("We good.Bye."))
}

const loadStorage = async ( path ) => {
    if (fs.existsSync(path)) {
        log(
            chalk.white(
                `Loading data from ${path}`
            )
        );
    } else {
        const msg = `Storage not found ${path}`;
        log(chalk.red(msg));
        process.exit(1);
    }

    return JSON.parse(fs.readFileSync(path, "utf8"));

}

const purgeBalancesLocks = async (source, destination) => {
    log("Purging balances locks - removing staking and democracy locks")
    const registry = new TypeRegistry();

    const storage = await loadStorage(source);

    let stakingAccounts = [];
    let democracyAccounts = [];

    const balanceLocksPrefix = modulePrefixes.get("Balances.Locks");

    if (!balanceLocksPrefix){
        log(chalk.red("Balance locks prefix not found in populated prefixes"))
        process.exit(1);
    }

    const adjusted = storage.map( (keyValue) => {

        const key = keyValue[0];
        const value = keyValue[1];

        let newValue = value;

        if (key.startsWith(balanceLocksPrefix)){
            let initialValue = newValue;
            newValue = removeStakingLocks(registry, value);
            if ( newValue !== initialValue){
                // Staking lock has been removed
                const address = hdxAddressFromKey(key, balanceLocksPrefix);
                stakingAccounts.push(address.toString());
                initialValue = newValue;
            }

            newValue = removeDemocracyLocks(registry, newValue);

            if ( newValue !== initialValue){
                // Staking lock has been removed
                const address = hdxAddressFromKey(key, balanceLocksPrefix);
                democracyAccounts.push(address.toString());
            }
        }

        return [key,newValue];
    })

    // I guess this could part of the previous statement
    const storageUpdatedWithoutEmptyLocks = adjusted.filter( (keyValue) => {
        const key = keyValue[0];
        const value = keyValue[1];

        if (key.startsWith(balanceLocksPrefix)){
            if (value === "0x00"){
                return false;
            }
        }
        return true;
    })


    fs.writeFileSync(destination, JSON.stringify(storageUpdatedWithoutEmptyLocks));
    log(`Purged ${chalk.yellow(stakingAccounts.length)} staking locks`)
    log(`Purged ${chalk.yellow(democracyAccounts.length)} democracy locks`)
    return [stakingAccounts, democracyAccounts];
}

const removeStakingLocks = (registry, value) => {
    let stakingId = registry.createType("LockIdentifier", "staking ");

    let locks = registry.createType("Vec<BalanceLock<Balance>>", value);

    const updateLocks = locks.filter( (lock) => lock.id.toString() !== stakingId.toString());

    return registry.createType("Vec<BalanceLock<Balance>>", updateLocks).toHex();
}

const removeDemocracyLocks = (registry, value) => {
    let stakingId = registry.createType("LockIdentifier", "democrac");

    let locks = registry.createType("Vec<BalanceLock<Balance>>", value);

    const updateLocks = locks.filter( (lock) => lock.id.toString() !== stakingId.toString());

    return registry.createType("Vec<BalanceLock<Balance>>", updateLocks).toHex();
}

const tripleBalance = (registry, value, decreaseConsumers) => {
    let aInfo = registry.createType("AccountInfo", value);
    const balance = new BN(aInfo.data.free.toString())
    const reserved = new BN(aInfo.data.reserved.toString())

    const issued = balance.muln(2).add(reserved.muln(2));

    const newBalance = balance.add(issued);

    let b = registry.createType("Balance", newBalance.toString(10,0));

    let newData = registry.createType("AccountData", { free: b,
        reserved: aInfo.data.reserved,
        miscFrozen: aInfo.data.miscFrozen,
        feeFrozen: aInfo.data.feeFrozen})

    let consumers = aInfo.consumers - decreaseConsumers;

    if ( consumers < 0){
        log(chalk.red("Consumers decreased below 0"))
        process.exit(1)
    }

    let newInfo = registry.createType("AccountInfo", { ...aInfo, nonce: 0, consumers: consumers,  data: newData});

    return [newInfo.toHex(), issued];
}

const tripleClaim = (registry, value) => {
    let newValue = value;

    if (value > 0 ) {
        let balanceBN = new BN(value.substring(2), 16, "le");
        balanceBN = balanceBN.imuln(3);
        let buffer = balanceBN.toBuffer("le", 16);
        newValue = "0x".concat(Buffer.from(buffer).toString("hex"));
    }

    return newValue;
}

const triple = async (source, destination, stakingAccountsRemoved = [], democracyLocksAccounts = []) => {
    const registry = new TypeRegistry();
    const storage = await loadStorage(source);

    const systemAccountPrefix = modulePrefixes.get("System.Account");
    const claimsPrefix = modulePrefixes.get("Claims.Claims");
    let totalIssued = new BN(0);

    const storageAdjusted = storage.map( ( keyValue ) => {
        const key = keyValue[0];
        const value = keyValue[1];

        let newValue = value;

        if ( key.startsWith(systemAccountPrefix)) {
            // Tripling balances
            const hdxAddress = hdxAddressFromKey(key, systemAccountPrefix);

            if ( ! excludeFromTripling.includes(hdxAddress) ) {
               let decreaseConsumers = 0;
                if ( stakingAccountsRemoved.includes(hdxAddress)){
                    decreaseConsumers += 1;
                }

                if ( democracyLocksAccounts.includes(hdxAddress)){
                    decreaseConsumers += 1;
                }

                const [tripled, issued] = tripleBalance(registry, value, decreaseConsumers);
                newValue = tripled;
                totalIssued = totalIssued.add(issued);
            }else{
                //TODO: staking locks and democracy locks for excluded address ?!!
                log(`Balance tripling - excluding ${chalk.yellow(hdxAddress)}`)
            }
        }else if (key.startsWith(claimsPrefix)){
            newValue = tripleClaim(registry, value);
        }

        return [key, newValue];
    }).map(([key, value]) => {
        let newValue = value;

        if (key.startsWith(modulePrefixes.get("Balances.TotalIssuance"))) {
            if (value > 0 ) {
                let totalIssuance = new BN(value.substring(2), 16, "le");
                totalIssuance = totalIssuance.add(totalIssued);
                const le = totalIssuance.toBuffer("le", 16);
                newValue = "0x".concat(Buffer.from(le).toString("hex"));
            }
        }

        return [key, newValue];
    });

    fs.writeFileSync(destination, JSON.stringify(storageAdjusted));
    log(`Balance and claims tripled. Stored in ${destination}`);
}


async function main() {

    const argv = yargs(hideBin(process.argv))
        .command('download', 'Download data for given block number', {
            block: {
                description: 'block number',
                alias: 'b',
                type: 'number'
            }
        })
        .command('validate', 'Validate balances and claims', {
        })
        .command('migrate', 'Perform migration', {
        })
        .command('prepare', 'Prepare storage - remove locks, triple balances and claims', {
        })

        .option('--dry-run', {
            description: 'Process and generate batch files. Exit before sending transactions',
            type: 'boolean'
        })
        .help()
        .alias('help', 'h').argv;

    await prefixes(SOURCE_RPC);

    if (argv._.includes('download')) {
        await downloadData(SOURCE_RPC, storagePath, argv.block);
        process.exit();
    }

    if (argv._.includes('validate')) {
        await validate(SOURCE_RPC, TARGET_RPC);
        process.exit();
    }

    if (argv._.includes('prepare')) {
        const [stakingLocksAccounts, democracyLocksAccounts] = await purgeBalancesLocks(storagePath, tempStoragePath);
        await triple(tempStoragePath, finalStoragePath, stakingLocksAccounts, democracyLocksAccounts);
        process.exit();
    }

    if (argv._.includes('migrate')) {
        if (fs.existsSync(finalStoragePath)) {
            log(
                chalk.white(
                    "Using ./data/finalStorage.json"
                )
            );
        } else {
            const msg = `Storage not found ${finalStoragePath}`;
            log(chalk.red(msg));
            process.exit(1);
        }

        const keyring = new Keyring({type: "sr25519"})
        const api = await createClient(TARGET_RPC);

        const [chain, nodeVersion] = await Promise.all([
            api.rpc.system.chain(),
            api.rpc.system.version(),
        ])

        log(chalk.green(`connected to ${TARGET_RPC} (${chain} ${nodeVersion}))`));

        const from = keyring.addFromUri(ACCOUNT_SECRET);

        const storage = JSON.parse(fs.readFileSync(finalStoragePath, "utf8"));

        log(`Key-value pairs to insert: ${chalk.yellow(storage.length)}`)

        const storageUpdates = asSingleUpdates(api, storage);
        const batch = api.tx.utility.batch(storageUpdates);

        let batch_calls = api.consts.utility.batchedCallsLimit;
        log(`Max call in one batch: ${batch_calls}`);

        let {maxExtrinsic: weightLimit} = api.consts.system.blockWeights.perClass.normal;
        log("Getting weight info of the whole batch")
        const {weight} = await batch.paymentInfo(from);
        log(`Weight of the whole batch: ${weight.toHuman()}`);
        log(`Weight limit: ${weightLimit.toHuman()}`);
        weightLimit = new BN(weightLimit.toString());

        const blocks = weight.div(weightLimit).toNumber() + 1;
        log(`Batch have to be split into ${blocks} blocks`);

         //utility batch can have only so many calls, so let's check if the split contains > max limit
        const updatesPerBlock = Math.ceil(storageUpdates.length / blocks);
        if (updatesPerBlock > batch_calls){
            log(chalk.red(`Max calls in batch exceeded`));
            process.exit(1);
        }

        const chunks = chunkify(storageUpdates, updatesPerBlock)
            .map(updates => api.tx.utility.batch(updates));

        log(`Splitting into ${blocks} chunks + verifying weight limit`)

        const weights = await Promise.all(
            chunks.map(async chunk => {
                const {weight} = await chunk.paymentInfo(from);
                assert(weight.lt(weightLimit), `chunk overweight: ${weight}`);
                return weight;
            })
        );
        log(`Chunk weights: ${weights}`);

        const startFrom = 0;

        if (argv.dryRun) {
            log(chalk.yellowBright("Dry-run. Exiting ..."));
            process.exit();
        }

        log(chalk.yellowBright("Sending batch transactions ...."));

        for (let idx = startFrom; idx < chunks.length; idx++) {
            log(`Batch ${idx}`);
            const {events} = await sendAndWaitFinalization({
                from,
                tx: asSudo(api, chunks[idx]),
                printEvents: ["sudo"]
            }).catch(e => {
                log(chalk.red(e));
                process.exit(1);
            });
        }

        process.exit();
    }

    log("No command selected");

    process.exit(0)
}

main().catch((e) => {
    console.error(e)
    process.exit()
})

