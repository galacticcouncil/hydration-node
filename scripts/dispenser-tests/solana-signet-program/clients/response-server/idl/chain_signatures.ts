/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/chain_signatures.json`.
 */
export type ChainSignatures = {
  address: 'H5tHfpYoEnarrrzcV7sWBcZhiKMvL2aRpUYvb1ydWkwS';
  metadata: {
    name: 'chainSignatures';
    version: '0.4.0';
    spec: '0.1.0';
    description: 'Chain signatures program for cross-chain signing on Solana';
    repository: 'https://github.com/sig-net/signet-solana-program';
  };
  instructions: [
    {
      name: 'getSignatureDeposit';
      docs: [
        '* @dev Function to get the current signature deposit amount.\n     * @return The current signature deposit amount.',
      ];
      discriminator: [45, 243, 86, 86, 58, 57, 172, 253];
      accounts: [
        {
          name: 'programState';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  112,
                  114,
                  111,
                  103,
                  114,
                  97,
                  109,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101,
                ];
              },
            ];
          };
        },
      ];
      args: [];
      returns: 'u64';
    },
    {
      name: 'initialize';
      docs: [
        '* @dev Function to initialize the program state.\n     * @param signature_deposit The deposit required for signature requests.\n     * @param chain_id The CAIP-2 chain identifier.',
      ];
      discriminator: [175, 175, 109, 31, 13, 152, 155, 237];
      accounts: [
        {
          name: 'programState';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  112,
                  114,
                  111,
                  103,
                  114,
                  97,
                  109,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101,
                ];
              },
            ];
          };
        },
        {
          name: 'admin';
          writable: true;
          signer: true;
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        },
      ];
      args: [
        {
          name: 'signatureDeposit';
          type: 'u64';
        },
        {
          name: 'chainId';
          type: 'string';
        },
      ];
    },
    {
      name: 'respond';
      docs: [
        '* @dev Function to respond to signature requests.\n     * @param request_ids The array of request IDs.\n     * @param signatures The array of signature responses.',
      ];
      discriminator: [72, 65, 227, 97, 42, 255, 147, 12];
      accounts: [
        {
          name: 'responder';
          signer: true;
        },
        {
          name: 'eventAuthority';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121,
                ];
              },
            ];
          };
        },
        {
          name: 'program';
        },
      ];
      args: [
        {
          name: 'requestIds';
          type: {
            vec: {
              array: ['u8', 32];
            };
          };
        },
        {
          name: 'signatures';
          type: {
            vec: {
              defined: {
                name: 'signature';
              };
            };
          };
        },
      ];
    },
    {
      name: 'respondBidirectional';
      docs: [
        '* @dev Function to finalize bidirectional flow\n     * @param request_id The ID of the signature request to respond to\n     * @param serialized_output output of the previously executed transaction\n     * @param signature ECDSA signature of the serialized output and request_id (keccak256(request_id.concat(serialized_output)))',
      ];
      discriminator: [138, 0, 45, 246, 236, 211, 109, 81];
      accounts: [
        {
          name: 'responder';
          signer: true;
        },
      ];
      args: [
        {
          name: 'requestId';
          type: {
            array: ['u8', 32];
          };
        },
        {
          name: 'serializedOutput';
          type: 'bytes';
        },
        {
          name: 'signature';
          type: {
            defined: {
              name: 'signature';
            };
          };
        },
      ];
    },
    {
      name: 'respondError';
      docs: [
        '* @dev Function to emit signature generation errors.\n     * @param errors The array of signature generation errors.',
      ];
      discriminator: [3, 170, 41, 132, 72, 184, 252, 69];
      accounts: [
        {
          name: 'responder';
          signer: true;
        },
      ];
      args: [
        {
          name: 'errors';
          type: {
            vec: {
              defined: {
                name: 'errorResponse';
              };
            };
          };
        },
      ];
    },
    {
      name: 'sign';
      docs: [
        '* @dev Function to request a signature.\n     * @param payload The payload to be signed.\n     * @param key_version The version of the key used for signing.\n     * @param path The derivation path for the user account.\n     * @param algo The algorithm used for signing.\n     * @param dest The response destination.\n     * @param params Additional parameters.',
      ];
      discriminator: [5, 221, 155, 46, 237, 91, 28, 236];
      accounts: [
        {
          name: 'programState';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  112,
                  114,
                  111,
                  103,
                  114,
                  97,
                  109,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101,
                ];
              },
            ];
          };
        },
        {
          name: 'requester';
          writable: true;
          signer: true;
        },
        {
          name: 'feePayer';
          writable: true;
          signer: true;
          optional: true;
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        },
        {
          name: 'eventAuthority';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121,
                ];
              },
            ];
          };
        },
        {
          name: 'program';
        },
      ];
      args: [
        {
          name: 'payload';
          type: {
            array: ['u8', 32];
          };
        },
        {
          name: 'keyVersion';
          type: 'u32';
        },
        {
          name: 'path';
          type: 'string';
        },
        {
          name: 'algo';
          type: 'string';
        },
        {
          name: 'dest';
          type: 'string';
        },
        {
          name: 'params';
          type: 'string';
        },
      ];
    },
    {
      name: 'signBidirectional';
      docs: [
        '* @dev Function to initiate bidirectional flow\n     * @param serialized_transaction transaction to be signed\n     * @param caip2_id chain identifier\n     * @param key_version The version of the key used for signing.\n     * @param path The derivation path for the user account.\n     * @param algo The algorithm used for signing.\n     * @param dest The response destination.\n     * @param params Additional parameters.\n     * @param program_id Program ID to callback after execution (not yet enabled).\n     * @param output_deserialization_schema schema for transaction output deserialization\n     * @param respond_serialization_schema serialization schema for respond_bidirectional payload',
      ];
      discriminator: [21, 104, 182, 213, 189, 143, 219, 48];
      accounts: [
        {
          name: 'programState';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  112,
                  114,
                  111,
                  103,
                  114,
                  97,
                  109,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101,
                ];
              },
            ];
          };
        },
        {
          name: 'requester';
          writable: true;
          signer: true;
        },
        {
          name: 'feePayer';
          writable: true;
          signer: true;
          optional: true;
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        },
        {
          name: 'instructions';
          optional: true;
        },
        {
          name: 'eventAuthority';
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  95,
                  95,
                  101,
                  118,
                  101,
                  110,
                  116,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121,
                ];
              },
            ];
          };
        },
        {
          name: 'program';
        },
      ];
      args: [
        {
          name: 'serializedTransaction';
          type: 'bytes';
        },
        {
          name: 'caip2Id';
          type: 'string';
        },
        {
          name: 'keyVersion';
          type: 'u32';
        },
        {
          name: 'path';
          type: 'string';
        },
        {
          name: 'algo';
          type: 'string';
        },
        {
          name: 'dest';
          type: 'string';
        },
        {
          name: 'params';
          type: 'string';
        },
        {
          name: 'programId';
          type: 'pubkey';
        },
        {
          name: 'outputDeserializationSchema';
          type: 'bytes';
        },
        {
          name: 'respondSerializationSchema';
          type: 'bytes';
        },
      ];
    },
    {
      name: 'updateDeposit';
      docs: [
        '* @dev Function to set the signature deposit amount.\n     * @param new_deposit The new deposit amount.',
      ];
      discriminator: [126, 116, 15, 164, 238, 179, 155, 59];
      accounts: [
        {
          name: 'programState';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  112,
                  114,
                  111,
                  103,
                  114,
                  97,
                  109,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101,
                ];
              },
            ];
          };
        },
        {
          name: 'admin';
          writable: true;
          signer: true;
          relations: ['programState'];
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        },
      ];
      args: [
        {
          name: 'newDeposit';
          type: 'u64';
        },
      ];
    },
    {
      name: 'withdrawFunds';
      docs: [
        '* @dev Function to withdraw funds from the program.\n     * @param amount The amount to withdraw.',
      ];
      discriminator: [241, 36, 29, 111, 208, 31, 104, 217];
      accounts: [
        {
          name: 'programState';
          writable: true;
          pda: {
            seeds: [
              {
                kind: 'const';
                value: [
                  112,
                  114,
                  111,
                  103,
                  114,
                  97,
                  109,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101,
                ];
              },
            ];
          };
        },
        {
          name: 'admin';
          writable: true;
          signer: true;
          relations: ['programState'];
        },
        {
          name: 'recipient';
          docs: ['function by checking it is not the zero address.'];
          writable: true;
        },
        {
          name: 'systemProgram';
          address: '11111111111111111111111111111111';
        },
      ];
      args: [
        {
          name: 'amount';
          type: 'u64';
        },
      ];
    },
  ];
  accounts: [
    {
      name: 'programState';
      discriminator: [77, 209, 137, 229, 149, 67, 167, 230];
    },
  ];
  events: [
    {
      name: 'depositUpdatedEvent';
      discriminator: [215, 193, 53, 27, 221, 101, 249, 108];
    },
    {
      name: 'fundsWithdrawnEvent';
      discriminator: [86, 232, 194, 4, 211, 69, 172, 202];
    },
    {
      name: 'respondBidirectionalEvent';
      discriminator: [195, 195, 28, 1, 102, 100, 189, 234];
    },
    {
      name: 'signBidirectionalEvent';
      discriminator: [135, 205, 217, 152, 96, 187, 11, 124];
    },
    {
      name: 'signatureErrorEvent';
      discriminator: [42, 28, 210, 105, 9, 196, 189, 51];
    },
    {
      name: 'signatureRequestedEvent';
      discriminator: [171, 129, 105, 91, 154, 49, 160, 34];
    },
    {
      name: 'signatureRespondedEvent';
      discriminator: [118, 146, 248, 151, 194, 93, 18, 86];
    },
  ];
  errors: [
    {
      code: 6000;
      name: 'insufficientDeposit';
      msg: 'Insufficient deposit amount';
    },
    {
      code: 6001;
      name: 'invalidInputLength';
      msg: 'Arrays must have the same length';
    },
    {
      code: 6002;
      name: 'unauthorized';
      msg: 'Unauthorized access';
    },
    {
      code: 6003;
      name: 'insufficientFunds';
      msg: 'Insufficient funds for withdrawal';
    },
    {
      code: 6004;
      name: 'invalidRecipient';
      msg: 'Invalid recipient address';
    },
    {
      code: 6005;
      name: 'invalidTransaction';
      msg: 'Invalid transaction data';
    },
    {
      code: 6006;
      name: 'missingInstructionSysvar';
      msg: 'Missing instruction sysvar';
    },
  ];
  types: [
    {
      name: 'affinePoint';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'x';
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'y';
            type: {
              array: ['u8', 32];
            };
          },
        ];
      };
    },
    {
      name: 'depositUpdatedEvent';
      docs: [
        '* @dev Emitted when the deposit amount is updated.\n * @param old_deposit The previous deposit amount.\n * @param new_deposit The new deposit amount.',
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'oldDeposit';
            type: 'u64';
          },
          {
            name: 'newDeposit';
            type: 'u64';
          },
        ];
      };
    },
    {
      name: 'errorResponse';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'requestId';
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'errorMessage';
            type: 'string';
          },
        ];
      };
    },
    {
      name: 'fundsWithdrawnEvent';
      docs: [
        '* @dev Emitted when a withdrawal is made.\n * @param amount The amount withdrawn.\n * @param recipient The address of the recipient.',
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'amount';
            type: 'u64';
          },
          {
            name: 'recipient';
            type: 'pubkey';
          },
        ];
      };
    },
    {
      name: 'programState';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'admin';
            type: 'pubkey';
          },
          {
            name: 'signatureDeposit';
            type: 'u64';
          },
          {
            name: 'chainId';
            type: 'string';
          },
        ];
      };
    },
    {
      name: 'respondBidirectionalEvent';
      docs: [
        '* @dev Emitted when a read response is received.\n * @param request_id The ID of the request. Must be calculated off-chain.\n * @param responder The address of the responder.\n * @param serialized_output The serialized output.\n * @param signature The signature.',
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'requestId';
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'responder';
            type: 'pubkey';
          },
          {
            name: 'serializedOutput';
            type: 'bytes';
          },
          {
            name: 'signature';
            type: {
              defined: {
                name: 'signature';
              };
            };
          },
        ];
      };
    },
    {
      name: 'signBidirectionalEvent';
      docs: [
        '* @dev Emitted when a sign_bidirectional request is made.\n * @param sender The address of the sender.\n * @param serialized_transaction The serialized transaction to be signed.\n * @param caip2_id The SLIP-44 chain ID.\n * @param key_version The version of the key used for signing.\n * @param deposit The deposit amount.\n * @param path The derivation path for the user account.\n * @param algo The algorithm used for signing.\n * @param dest The response destination.\n * @param params Additional parameters.\n * @param program_id Program ID to callback after execution (not yet enabled).\n * @param output_deserialization_schema Schema for transaction output deserialization.\n * @param respond_serialization_schema Serialization schema for respond_bidirectional payload.',
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'sender';
            type: 'pubkey';
          },
          {
            name: 'serializedTransaction';
            type: 'bytes';
          },
          {
            name: 'caip2Id';
            type: 'string';
          },
          {
            name: 'keyVersion';
            type: 'u32';
          },
          {
            name: 'deposit';
            type: 'u64';
          },
          {
            name: 'path';
            type: 'string';
          },
          {
            name: 'algo';
            type: 'string';
          },
          {
            name: 'dest';
            type: 'string';
          },
          {
            name: 'params';
            type: 'string';
          },
          {
            name: 'programId';
            type: 'pubkey';
          },
          {
            name: 'outputDeserializationSchema';
            type: 'bytes';
          },
          {
            name: 'respondSerializationSchema';
            type: 'bytes';
          },
        ];
      };
    },
    {
      name: 'signature';
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'bigR';
            type: {
              defined: {
                name: 'affinePoint';
              };
            };
          },
          {
            name: 's';
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'recoveryId';
            type: 'u8';
          },
        ];
      };
    },
    {
      name: 'signatureErrorEvent';
      docs: [
        '* @dev Emitted when a signature error is received.\n * @notice Any address can emit this event. Do not rely on it for business logic.\n * @param request_id The ID of the request. Must be calculated off-chain.\n * @param responder The address of the responder.\n * @param error The error message.',
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'requestId';
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'responder';
            type: 'pubkey';
          },
          {
            name: 'error';
            type: 'string';
          },
        ];
      };
    },
    {
      name: 'signatureRequestedEvent';
      docs: [
        '* @dev Emitted when a signature is requested.\n * @param sender The address of the sender.\n * @param payload The payload to be signed.\n * @param key_version The version of the key used for signing.\n * @param deposit The deposit amount.\n * @param chain_id The CAIP-2 ID of the blockchain.\n * @param path The derivation path for the user account.\n * @param algo The algorithm used for signing.\n * @param dest The response destination.\n * @param params Additional parameters.\n * @param fee_payer Optional fee payer account.',
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'sender';
            type: 'pubkey';
          },
          {
            name: 'payload';
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'keyVersion';
            type: 'u32';
          },
          {
            name: 'deposit';
            type: 'u64';
          },
          {
            name: 'chainId';
            type: 'string';
          },
          {
            name: 'path';
            type: 'string';
          },
          {
            name: 'algo';
            type: 'string';
          },
          {
            name: 'dest';
            type: 'string';
          },
          {
            name: 'params';
            type: 'string';
          },
          {
            name: 'feePayer';
            type: {
              option: 'pubkey';
            };
          },
        ];
      };
    },
    {
      name: 'signatureRespondedEvent';
      docs: [
        '* @dev Emitted when a signature response is received.\n * @notice Any address can emit this event. Clients should always verify the validity of the signature.\n * @param request_id The ID of the request. Must be calculated off-chain.\n * @param responder The address of the responder.\n * @param signature The signature response.',
      ];
      type: {
        kind: 'struct';
        fields: [
          {
            name: 'requestId';
            type: {
              array: ['u8', 32];
            };
          },
          {
            name: 'responder';
            type: 'pubkey';
          },
          {
            name: 'signature';
            type: {
              defined: {
                name: 'signature';
              };
            };
          },
        ];
      };
    },
  ];
};
