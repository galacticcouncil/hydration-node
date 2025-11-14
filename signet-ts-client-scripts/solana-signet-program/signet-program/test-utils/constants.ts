export const EPSILON_DERIVATION_PREFIX =
  "sig.network v1.0.0 epsilon derivation";
export const SOLANA_CHAIN_ID = "0x800001f5";
export const SECP256K1_CURVE_ORDER = BigInt(
  "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141"
);

// CPI Event Structure:
// When emit_cpi! is used in an Anchor program, it creates an instruction with this format:
// [0-8]:   EMIT_CPI_INSTRUCTION_DISCRIMINATOR - identifies this as an emit_cpi! instruction
// [8-16]:  Event discriminator - identifies which specific event (from IDL)
// [16+]:   Event data - the serialized event fields

// EMIT_CPI_INSTRUCTION_DISCRIMINATOR - identifies that this is an emit_cpi! instruction
// This is a constant from Anchor that identifies the instruction type
// Value: e445a52e51cb9a1d
export const ANCHOR_EMIT_CPI_CALL_BACK_DISCRIMINATOR = Buffer.from([
  0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d,
]);

export const eventNames = {
  signatureRequested: "signatureRequestedEvent",
  signatureResponded: "signatureRespondedEvent",
} as const;
