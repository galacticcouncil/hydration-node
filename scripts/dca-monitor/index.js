import {ApiPromise, WsProvider, Keyring} from "@polkadot/api";

const ENDPOINT = 'ws://localhost:8000';
const BLOCK_COUNT = 150;

async function main() {
    console.log(`Connecting to ${ENDPOINT}...`);
    const provider = new WsProvider(ENDPOINT);
    const api = await ApiPromise.create({
        provider,
        throwOnConnect: false,
        throwOnError: false,
    });

    const chain = await api.rpc.system.chain();
    console.log(`Connected to ${chain}`);

    const keyring = new Keyring({type: 'sr25519'});
    const alice = keyring.addFromUri('//Alice');
    console.log(`Using account: ${alice.address}`);

    // Per-schedule tracker: Map<ScheduleId, { events[], status, failureDetails, who }>
    const schedules = new Map();
    const randomnessFailures = [];
    const reserveUnlocks = [];

    // Non-DCA event trackers
    const extrinsicFailures = [];
    const routerExecutions = [];
    const swaps = [];
    const dustLost = [];

    const getSchedule = (id) => {
        if (!schedules.has(id)) {
            schedules.set(id, {
                events: [],
                status: 'active',
                failureDetails: [],
                who: null,
            });
        }
        return schedules.get(id);
    };

    const truncate = (addr) => {
        if (!addr) return 'N/A';
        const s = addr.toString();
        return s.length > 16 ? `${s.slice(0, 8)}...${s.slice(-6)}` : s;
    };

    const decodeError = (dispatchError) => {
        if (dispatchError.isModule) {
            const decoded = api.registry.findMetaError(dispatchError.asModule);
            return `${decoded.section}.${decoded.method}`;
        }
        return dispatchError.toString();
    };

    let nonce = (await api.rpc.system.accountNextIndex(alice.address)).toNumber();

    console.log(`\nProducing ${BLOCK_COUNT} blocks and monitoring DCA events...\n`);

    for (let i = 0; i < BLOCK_COUNT; i++) {
        const blockHash = await new Promise((resolve, reject) => {
            api.tx.system.remark('dca-monitor').signAndSend(alice, {nonce: nonce++}, (receipt) => {
                if (receipt.status.isInBlock) {
                    resolve(receipt.status.asInBlock);
                }
            }).catch(reject);
        });

        const blockNumber = (await api.rpc.chain.getHeader(blockHash)).number.toNumber();

        // Query ALL block events (DCA fires in on_initialize, not in extrinsic receipts)
        const events = await api.query.system.events.at(blockHash);

        let dcaCount = 0;
        let otherCount = 0;
        for (const record of events) {
            const {event} = record;
            const section = event.section;
            const method = event.method;
            const data = event.data;

            if (section === 'dca') {
                dcaCount++;
                switch (method) {
                    case 'ExecutionStarted': {
                        const id = data[0].toString();
                        const schedule = getSchedule(id);
                        schedule.events.push({method, block: blockNumber});
                        break;
                    }
                    case 'Scheduled': {
                        const id = data[0].toString();
                        const who = data[1].toString();
                        const schedule = getSchedule(id);
                        schedule.who = who;
                        schedule.events.push({method, block: blockNumber});
                        break;
                    }
                    case 'ExecutionPlanned': {
                        const id = data[0].toString();
                        const who = data[1].toString();
                        const schedule = getSchedule(id);
                        schedule.who = schedule.who || who;
                        schedule.events.push({method, block: blockNumber});
                        break;
                    }
                    case 'TradeExecuted': {
                        const id = data[0].toString();
                        const who = data[1].toString();
                        const amountIn = data[2].toString();
                        const amountOut = data[3].toString();
                        const schedule = getSchedule(id);
                        schedule.who = schedule.who || who;
                        schedule.events.push({method, block: blockNumber, amountIn, amountOut});
                        break;
                    }
                    case 'TradeFailed': {
                        const id = data[0].toString();
                        const who = data[1].toString();
                        const error = decodeError(data[2]);
                        const schedule = getSchedule(id);
                        schedule.who = schedule.who || who;
                        schedule.events.push({method, block: blockNumber, error});
                        schedule.failureDetails.push({block: blockNumber, error});
                        break;
                    }
                    case 'Terminated': {
                        const id = data[0].toString();
                        const who = data[1].toString();
                        const error = decodeError(data[2]);
                        const schedule = getSchedule(id);
                        schedule.who = schedule.who || who;
                        schedule.status = 'terminated';
                        schedule.events.push({method, block: blockNumber, error});
                        schedule.failureDetails.push({block: blockNumber, error: `TERMINATED: ${error}`});
                        break;
                    }
                    case 'Completed': {
                        const id = data[0].toString();
                        const who = data[1].toString();
                        const schedule = getSchedule(id);
                        schedule.who = schedule.who || who;
                        schedule.status = 'completed';
                        schedule.events.push({method, block: blockNumber});
                        break;
                    }
                    case 'RandomnessGenerationFailed': {
                        const block = data[0].toString();
                        const error = decodeError(data[1]);
                        randomnessFailures.push({block: blockNumber, error});
                        break;
                    }
                    case 'ReserveUnlocked': {
                        const who = data[0].toString();
                        const assetId = data[1].toString();
                        reserveUnlocks.push({who, assetId, block: blockNumber});
                        break;
                    }
                    default:
                        console.log(`  Unknown DCA event: ${method}`);
                }
            } else if (section === 'system' && method === 'ExtrinsicFailed') {
                otherCount++;
                const dispatchError = data[0];
                extrinsicFailures.push({block: blockNumber, error: decodeError(dispatchError)});
            } else if (section === 'router' && method === 'Executed') {
                otherCount++;
                routerExecutions.push({
                    block: blockNumber,
                    assetIn: data[0].toString(),
                    assetOut: data[1].toString(),
                    amountIn: data[2].toString(),
                    amountOut: data[3].toString(),
                });
            } else if (section === 'broadcast' && method === 'Swapped3') {
                otherCount++;
                swaps.push({block: blockNumber, swapper: data[0].toString()});
            } else if (section === 'tokens' && method === 'DustLost') {
                otherCount++;
                dustLost.push({
                    block: blockNumber,
                    who: data[0].toString(),
                    assetId: data[1].toString(),
                    amount: data[2].toString(),
                });
            }
        }

        const parts = [];
        if (dcaCount > 0) parts.push(`${dcaCount} DCA`);
        if (otherCount > 0) parts.push(`${otherCount} other`);
        console.log(`Block #${blockNumber} (${i + 1}/${BLOCK_COUNT}): ${parts.length > 0 ? parts.join(', ') : 'no tracked events'}`);
    }

    // === Output Summary ===
    console.log('\n' + '='.repeat(80));
    console.log('DCA MONITOR SUMMARY');
    console.log('='.repeat(80));

    if (schedules.size === 0) {
        console.log('\nNo DCA schedule events detected in the monitored blocks.');
    } else {
        const tableData = [];
        for (const [id, info] of schedules) {
            const executions = info.events.filter(e => e.method === 'ExecutionStarted').length;
            const tradesOk = info.events.filter(e => e.method === 'TradeExecuted').length;
            const tradesFailed = info.events.filter(e => e.method === 'TradeFailed').length;
            const lastTrade = [...info.events].reverse().find(e => e.method === 'TradeExecuted' || e.method === 'TradeFailed');
            tableData.push({
                'Schedule ID': id,
                'Owner': truncate(info.who),
                'Executions': executions,
                'Trades OK': tradesOk,
                'Trades Failed': tradesFailed,
                'Last Trade': lastTrade ? (lastTrade.method === 'TradeExecuted' ? 'OK' : 'FAILED') : '-',
                'Status': info.status,
            });
        }
        console.log('\nSchedule Summary:');
        console.table(tableData);

        // Print failure details
        const allFailures = [];
        for (const [id, info] of schedules) {
            for (const f of info.failureDetails) {
                allFailures.push({'Schedule ID': id, 'Block': f.block, 'Error': f.error});
            }
        }
        if (allFailures.length > 0) {
            console.log('\nFailure Details:');
            console.table(allFailures);
        }
    }

    if (randomnessFailures.length > 0) {
        console.log('\nRandomness Generation Failures:');
        console.table(randomnessFailures);
    }

    if (reserveUnlocks.length > 0) {
        console.log('\nReserve Unlocks:');
        console.table(reserveUnlocks.map(r => ({
            'Owner': truncate(r.who),
            'Asset ID': r.assetId,
            'Block': r.block,
        })));
    }

    // === Non-DCA Events ===
    console.log('\n' + '='.repeat(80));
    console.log('OTHER EVENTS');
    console.log('='.repeat(80));

    console.log(`\nRouter Executions: ${routerExecutions.length}`);
    console.log(`Broadcast Swaps: ${swaps.length}`);
    console.log(`Extrinsic Failures: ${extrinsicFailures.length}`);
    console.log(`Dust Lost: ${dustLost.length}`);

    if (extrinsicFailures.length > 0) {
        console.log('\nExtrinsic Failure Details:');
        console.table(extrinsicFailures);
    }

    if (dustLost.length > 0) {
        console.log('\nDust Lost Details:');
        console.table(dustLost.map(d => ({
            'Block': d.block,
            'Who': truncate(d.who),
            'Asset ID': d.assetId,
            'Amount': d.amount,
        })));
    }

    // Final verdict
    const totalSchedules = schedules.size;
    const terminatedSchedules = [...schedules.values()].filter(s => s.status === 'terminated').length;
    const withTradeFailures = [...schedules.values()].filter(s => s.failureDetails.length > 0 && s.status !== 'terminated').length;

    console.log('\n' + '='.repeat(80));
    console.log('VERDICT');
    console.log('='.repeat(80));
    console.log(`DCA schedules: ${totalSchedules} observed — ${terminatedSchedules} terminated, ${withTradeFailures} with trade failures`);
    console.log(`Extrinsic failures: ${extrinsicFailures.length}`);
    console.log(`Dust lost events: ${dustLost.length}`);
    console.log('='.repeat(80));

    await api.disconnect();
    console.log('Disconnected.');
}

main()
    .catch(console.error)
    .finally(() => process.exit(0));
