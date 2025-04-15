const fs = require('fs');
const { TypeRegistry } = require('@polkadot/types');
const { hexToU8a, u8aToHex } = require('@polkadot/util');

// Define network names
const NEW_NAME = process.env.CHAIN_NAME || "Hydration Local Testnet";
const NEW_ID = process.env.CHAIN_ID || "local_testnet";
const NEW_RELAY_CHAIN = "rococo_local_testnet";

// Define replacement values
const AURA_AUTHORITIES_VALUE = "0x08be4f21c56d926b91f020b5071f14935cb93f001f1127c53d3eac6eed23ffea64dc4d79aad5a9d01a359995838830a80733a0bff7e4eb087bfc621ef1873fec49";
const COUNCIL_AND_TECHNICAL_COMMITTEE_VALUE = "0x04d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d";
const SYSTEM_ACCOUNT_VALUE = "0x00000000000000000100000000000000ba31bc09df123864f700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

async function updateChainSpec(inputFile, outputFile) {
    if (fs.existsSync(outputFile)) {
        console.log(`Output file ${outputFile} already exists, skipping processing...`);
        return;
    }

    console.log('Starting the chain spec update script...');
    let chainSpec;
    try {
        chainSpec = JSON.parse(fs.readFileSync(inputFile, 'utf8'));
    } catch (err) {
        console.error('Error reading the chain spec file:', err);
        process.exit(1);
    }

    // Create a new registry with custom types
    const registry = new TypeRegistry();
    registry.register({
        EmaOracleEntry: {
            price: { n: 'u128', d: 'u128' },
            volume: { aIn: 'u128', bOut: 'u128', aOut: 'u128', bIn: 'u128' },
            liquidity: { a: 'u128', b: 'u128' },
            updatedAt: 'u64',
        },
        PalletLiquidityMiningFarmState: {
            _enum: ['Active', 'Terminated']
        },
        Perquintill: 'u64',
        FixedU128: 'u128',
        AccountId32: '[u8; 32]',
        PalletLiquidityMiningGlobalFarmData: {
            id: 'u32',
            owner: 'AccountId32',
            updatedAt: 'u32',
            totalSharesZ: 'u128',
            accumulatedRpz: 'FixedU128',
            rewardCurrency: 'u32',
            pendingRewards: 'u128',
            accumulatedPaidRewards: 'u128',
            yieldPerPeriod: 'Perquintill',
            plannedYieldingPeriods: 'u32',
            blocksPerPeriod: 'u32',
            incentivizedAsset: 'u32',
            maxRewardPerPeriod: 'u128',
            minDeposit: 'u128',
            liveYieldFarmsCount: 'u32',
            totalYieldFarmsCount: 'u32',
            priceAdjustment: 'FixedU128',
            state: 'PalletLiquidityMiningFarmState'
        }
    });

    const governance = process.env.KEEP_GOVERNANCE ? {} : {
        "0xaebd463ed9925c488c112434d61debc0ba7fb8745735dc3be2a2c61a72c39e78": COUNCIL_AND_TECHNICAL_COMMITTEE_VALUE, // Council.members
        "0xed25f63942de25ac5253ba64b5eb64d1ba7fb8745735dc3be2a2c61a72c39e78": COUNCIL_AND_TECHNICAL_COMMITTEE_VALUE, // TechnicalCommittee.members
    }

    const deployer = process.env.NO_DEPLOYER ? {} : {
        "0x2c2b3fbb4fc221c42de8259db454678fe74405d2678f6b81824443771f6fa86af065818ad972112ab4e06ea85b354e36222222ff7be76052e023ec1a306fcca8f9659d80": "0x", // Contract deployer 0x222222ff7Be76052e023Ec1a306fCca8F9659D80
        "0x99971b5749ac43e0235e41b0d37869188ee7418a6531173d60d1f6a82d8f4d5173d3a4140c3587d7bc56f1a1c01a1c5e45544800222222ff7be76052e023ec1a306fcca8f9659d8000000000000000001f0e76f06ebd150314000000": "0x000064a7b3b6e00d00000000000000000000000000000000000000000000000000000000000000000000000000000000", // 1 ETH for 0x222222
    }

    // Define replacements
    const REPLACEMENTS = {
        "0x0d715f2646c8f85767b5d2764bb2782604a74d81251e398fd8a0a4d55023bb3f": "0xf2070000", // parachainInfo.parachainId = 2034
        "0x57f8dc2f5ab09467896f47300f0424385e0621c4869aa60c02be9adcc98a0d1d": AURA_AUTHORITIES_VALUE, // aura.authorities
        "0x3c311d57d4daf52904616cf69648081e5e0621c4869aa60c02be9adcc98a0d1d": AURA_AUTHORITIES_VALUE, // auraExt.authorities
        "0xcec5070d609dd3497f72bde07fc96ba088dcde934c658227ee1dfafcd6e16903": AURA_AUTHORITIES_VALUE, // Session validators
        "0x15464cac3378d46f113cd5b7a4d71c845579297f4dfb9609e7e4c2ebab9ce40a": AURA_AUTHORITIES_VALUE, // CollatorSelection.invulnerables
        ...governance,
        "0x26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da9de1e86a9a8c739864cf3cc5ec2bea59fd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d": SYSTEM_ACCOUNT_VALUE, // System account
        ...deployer,
    };

    // Define keys to delete
    const KEYS_TO_DELETE = [
        "0x45323df7cc47150b3930e2666b0aa313911a5dd3f1155f5b7d0c5aa102a757f9", // ParachainSystem.lastDmqMqcHead
        "0x45323df7cc47150b3930e2666b0aa3133dca42deb008c6559ee789c9b9f70a2c", // ParachainSystem.lastHrmpMqcHeads
        "0x45323df7cc47150b3930e2666b0aa313a2bca190d36bd834cc73a38fc213ecbd", // ParachainSystem.lastRelayChainBlockNumber
        "0x7cda3cfa86b349fdafce4979b197118f948ece45793d7f15c9c0b9574ddbc665", // Elections.CandidateQueue
        "0x7cda3cfa86b349fdafce4979b197118f7657ad2ff3a6742e1071bbb898ce5431", // Elections.Members
        "0x7cda3cfa86b349fdafce4979b197118fba7fb8745735dc3be2a2c61a72c39e78", // Elections.RunnersUp
        "0x7cda3cfa86b349fdafce4979b197118f40982df579bdf1315224f41e5f482063", // Elections.Votes
        "0x5258a12472693b34a3ed25509781e55f3ffefddfbe00a43e565ba6114d1589ea", // Elections.StakeOf
        "0xcec5070d609dd3497f72bde07fc96ba0e0cdd062e6eaf24295ad4ccfc41d4609", // Session.queuedKeys
        "0xcec5070d609dd3497f72bde07fc96ba072763800a36a99fdfc7c10f6415f6ee6", // Session.currentIndex
    ];

    // Define prefixes to delete
    const PREFIXES_TO_DELETE = [
        "0x7cda3cfa86b349fdafce4979b197118f71cd3068e6118bfb392b798317f63a89", // Elections.voting
        "0x5258a12472693b34a3ed25509781e55fb79", // emaOracle.accumulator
        "0xcec5070d609dd3497f72bde07fc96ba04c014e6bf8b8c2c011e7290b85696bb3", // Session.nextKeys
    ];

    KEYS_TO_DELETE.forEach((key) => delete chainSpec.genesis.raw.top[key]);

    // Process prefix-based deletions
    for (const prefix of PREFIXES_TO_DELETE) {
        for (const key of Object.keys(chainSpec.genesis.raw.top)) {
            if (key.startsWith(prefix)) {
                delete chainSpec.genesis.raw.top[key];
            }
        }
    }

    // Process EmaOracleEntry updates
    console.log('Processing EmaOracleEntry & GlobalFarm updates...');
    for (const [key, value] of Object.entries(chainSpec.genesis.raw.top)) {
        if (key.startsWith("0x5258a12472693b34a3ed25509781e55fb79")) {
            try {
                const decodedValue = registry.createType('EmaOracleEntry', hexToU8a(value));
                if (decodedValue.updatedAt !== undefined) {
                    decodedValue.updatedAt = 0; // Set updatedAt to 0
                    chainSpec.genesis.raw.top[key] = u8aToHex(decodedValue.toU8a());
                }
            } catch (err) {
                console.error(`Error processing EmaOracleEntry for key ${key}:`, err);
            }
        } else if (key.startsWith("0xa1a851f6ddab88c23c6615f42a0062df8d84255c07d18453a739a171ac5cf629")) {
            try {
                const decoded = registry.createType('PalletLiquidityMiningGlobalFarmData', hexToU8a(value));
                const json = decoded.toJSON();
                const id = json.id;
                json.updatedAt = 0;
                const updated = registry.createType('PalletLiquidityMiningGlobalFarmData', json);
                chainSpec.genesis.raw.top[key] = u8aToHex(updated.toU8a());
            } catch (err) {
                console.error(`Error processing globalFarm for key ${key}:`, err);
            }
        }
    }

    for (const [key, value] of Object.entries(REPLACEMENTS)) {
        chainSpec.genesis.raw.top[key] = value;
    }

    // Update metadata fields
    chainSpec.name = NEW_NAME;
    chainSpec.id = NEW_ID;
    chainSpec.relay_chain = NEW_RELAY_CHAIN;
    chainSpec.para_id = 2034;

    // Save the updated chain spec
    try {
        fs.writeFileSync(outputFile, JSON.stringify(chainSpec, null, 4));
        console.log(`Chain spec updated successfully and saved to ${outputFile}`);
    } catch (err) {
        console.error('Error writing the updated chain spec file:', err);
    }

    console.log('Chain spec update script completed.');
}

const inputFile = process.argv[2];
const outputFile = process.argv[3];

if (!inputFile || !outputFile) {
    console.error('Usage: node updateChainSpec.js <inputFile> <outputFile>');
    process.exit(1);
}

updateChainSpec(inputFile, outputFile).catch((error) => {
    console.error('Error updating chain spec:', error);
    process.exit(1);
});
