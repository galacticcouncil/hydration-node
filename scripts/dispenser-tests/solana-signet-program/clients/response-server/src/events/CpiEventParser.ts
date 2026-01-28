import * as anchor from '@coral-xyz/anchor';
import { Connection } from '@solana/web3.js';
import bs58 from 'bs58';
import { CpiEventData } from '../types';

// EMIT_CPI_INSTRUCTION_DISCRIMINATOR - identifies that this is an emit_cpi! instruction
// This is a constant from Anchor that identifies the instruction type
// Value: e445a52e51cb9a1d
export const EMIT_CPI_INSTRUCTION_DISCRIMINATOR = Buffer.from([
  0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d,
]);

export interface ParsedCpiEvent {
  name: string;
  data: CpiEventData;
}

export class CpiEventParser {
  /**
   * Parse CPI events from a transaction
   * @param connection Solana connection
   * @param signature Transaction signature
   * @param targetProgramId Program ID to filter events for
   * @param program Anchor program instance
   * @returns Array of parsed events
   */
  static async parseCpiEvents(
    connection: Connection,
    signature: string,
    targetProgramId: string,
    program: anchor.Program
  ): Promise<ParsedCpiEvent[]> {
    const events: ParsedCpiEvent[] = [];

    try {
      // Get the transaction with JsonParsed encoding to access inner instructions
      // CPI events appear as inner instructions when emit_cpi! is used
      const tx = await connection.getParsedTransaction(signature, {
        commitment: 'confirmed',
        maxSupportedTransactionVersion: 0,
      });

      if (!tx || !tx.meta) return events;

      const innerInstructions = tx.meta.innerInstructions || [];

      for (const innerIxSet of innerInstructions) {
        for (const instruction of innerIxSet.instructions) {
          if (
            'programId' in instruction &&
            'data' in instruction &&
            instruction.programId.toString() === targetProgramId
          ) {
            const parsedEvent = CpiEventParser.parseInstruction(
              instruction.data,
              program
            );
            if (parsedEvent) {
              events.push(parsedEvent);
            }
          }
        }
      }
    } catch (error) {
      console.error('Error parsing transaction for CPI events:', error);
    }

    return events;
  }

  /**
   * Parse a single instruction for CPI event data
   * @param instructionData Base58 encoded instruction data
   * @param program Anchor program instance
   * @returns Parsed event or null if not a CPI event
   */
  private static parseInstruction(
    instructionData: string,
    program: anchor.Program
  ): ParsedCpiEvent | null {
    try {
      const ixData = bs58.decode(instructionData);

      // Check if this is an emit_cpi! instruction
      // The instruction data format is:
      // [0-8]:   emit_cpi! instruction discriminator
      // [8-16]:  event discriminator (identifies which event type)
      // [16+]:   event data (the actual event fields)
      if (
        ixData.length >= 16 &&
        Buffer.compare(
          ixData.subarray(0, 8),
          EMIT_CPI_INSTRUCTION_DISCRIMINATOR
        ) === 0
      ) {
        // Extract the event discriminator (bytes 8-16)
        const eventDiscriminator = ixData.subarray(8, 16);

        // Extract the event data (after byte 16)
        const eventData = ixData.subarray(16);

        // Match the event discriminator against our IDL to identify the event type
        let matchedEvent = null;
        for (const event of program.idl.events || []) {
          // Convert the discriminator array from IDL to Buffer for comparison
          const idlDiscriminator = Buffer.from(event.discriminator);

          if (Buffer.compare(eventDiscriminator, idlDiscriminator) === 0) {
            matchedEvent = event;
            break;
          }
        }

        if (matchedEvent) {
          try {
            // Reconstruct the full event buffer for Anchor's BorshEventCoder
            // The coder expects: [event discriminator (8 bytes) + event data]
            const fullEventData = Buffer.concat([
              eventDiscriminator,
              eventData,
            ]);

            // Decode using Anchor's BorshEventCoder
            const eventCoder = new anchor.BorshEventCoder(program.idl);
            const decodedEvent = eventCoder.decode(
              fullEventData.toString('base64')
            );

            if (decodedEvent) {
              return decodedEvent;
            }
          } catch {
            // Ignore decode errors
          }
        }
      }
    } catch (e) {
      // Not our event, continue
    }

    return null;
  }

  /**
   * Subscribe to CPI events for a program
   * @param connection Solana connection
   * @param program Anchor program instance
   * @param eventHandlers Map of event names to handler functions
   * @returns Subscription ID for cleanup
   */
  static subscribeToCpiEvents(
    connection: Connection,
    program: anchor.Program,
    eventHandlers: Map<
      string,
      (event: CpiEventData, slot: number) => Promise<void>
    >
  ): number {
    return connection.onLogs(
      program.programId,
      async (logs, context) => {
        // Skip failed transactions - CPI events require valid transactions
        if (logs.err) {
          return;
        }

        // Parse CPI events from inner instructions
        const events = await CpiEventParser.parseCpiEvents(
          connection,
          logs.signature,
          program.programId.toString(),
          program
        );

        // Process each event with its corresponding handler
        for (const event of events) {
          const handler = eventHandlers.get(event.name);
          if (handler) {
            // Use the slot from context
            await handler(event.data, context.slot);
          }
        }
      },
      'confirmed'
    );
  }
}
