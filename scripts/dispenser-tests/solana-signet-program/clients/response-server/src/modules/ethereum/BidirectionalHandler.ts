import { RequestIdGenerator } from '../RequestIdGenerator';
import { EthereumTransactionProcessor } from './EthereumTransactionProcessor';
import type { SignBidirectionalEvent } from '../../types';
import type { BidirectionalHandlerContext } from '../shared/BidirectionalContext';

export async function handleEthereumBidirectional(
  event: SignBidirectionalEvent,
  context: BidirectionalHandlerContext,
  derivedPrivateKey: string
): Promise<void> {
  const { config, pendingTransactions, sendSignatures, source } = context;

  const senderStr =
    typeof event.sender === 'string' ? event.sender : event.sender.toString();

  const requestId = RequestIdGenerator.generateSignBidirectionalRequestId(
    senderStr,
    Array.from(event.serializedTransaction),
    event.caip2Id,
    event.keyVersion,
    event.path,
    event.algo,
    event.dest,
    event.params
  );

  console.log(`🔑 Request ID (eip155): ${requestId}`);

  const result =
    await EthereumTransactionProcessor.processTransactionForSigning(
      new Uint8Array(event.serializedTransaction),
      derivedPrivateKey,
      event.caip2Id,
      config
    );

  const requestIdBytes = Buffer.from(requestId.slice(2), 'hex');
  const requestIds = result.signature.map(() => requestIdBytes);

  await sendSignatures(requestIds, result.signature);

  console.log(`✅ Signatures sent to contract (tx)`);

  pendingTransactions.set(result.signedTxHash, {
    txHash: result.signedTxHash,
    requestId,
    caip2Id: event.caip2Id,
    explorerDeserializationSchema: Buffer.from(
      event.outputDeserializationSchema
    ),
    callbackSerializationSchema: Buffer.from(event.respondSerializationSchema),
    fromAddress: result.fromAddress,
    nonce: result.nonce,
    checkCount: 0,
    namespace: 'eip155',
    source,
  });

  console.log(`🔍 Monitoring transaction ${result.signedTxHash} (eip155)`);
}
