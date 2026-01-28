import * as borsh from 'borsh';
import type { Schema } from 'borsh';
import { ethers } from 'ethers';
import {
  TransactionOutputData,
  BorshSchema,
  AbiSchemaField,
  SerializableValue,
} from '../types';

export class OutputSerializer {
  static async serialize(
    output: TransactionOutputData,
    format: number,
    schema: Buffer | number[]
  ): Promise<Uint8Array> {
    if (format === 0) {
      return this.serializeBorsh(output, schema);
    } else if (format === 1) {
      return this.serializeAbi(output, schema);
    }

    throw new Error(`Unsupported serialization format: ${format}`);
  }

  private static async serializeBorsh(
    output: TransactionOutputData,
    schema: Buffer | number[]
  ): Promise<Uint8Array> {
    const schemaStr = this.getSchemaString(schema);
    const parsedSchema = JSON.parse(schemaStr);

    // Handle scalar bool schema (schema is literally "bool")
    if (typeof parsedSchema === 'string' && parsedSchema === 'bool') {
      const boolValue =
        typeof output === 'boolean'
          ? output
          : 'error' in (output as Record<string, unknown>)
            ? Boolean((output as any).error)
            : 'success' in (output as Record<string, unknown>)
              ? Boolean((output as any).success)
              : true;

      try {
        return borsh.serialize('bool' as unknown as Schema, boolValue);
      } catch (error) {
        console.error(
          '[OutputSerializer] Borsh serialization failed (scalar bool)',
          { schema: parsedSchema, payload: boolValue },
          error
        );
        throw error;
      }
    }

    const borshSchema = parsedSchema as BorshSchema;

    let dataToSerialize: SerializableValue = output;
    if (output.isFunctionCall === false) {
      dataToSerialize = this.createBorshData(borshSchema, output);
    }

    // Handle single-field objects with empty key
    if (typeof dataToSerialize === 'object' && dataToSerialize !== null) {
      const keys = Object.keys(dataToSerialize as TransactionOutputData);
      if (keys.length === 1 && keys[0] === '') {
        dataToSerialize = (dataToSerialize as TransactionOutputData)[''];
      }
    }

    // Wrap common boolean error struct when payload is an object with `error`
    if (
      borshSchema.struct &&
      Object.keys(borshSchema.struct).length === 1 &&
      borshSchema.struct.error === 'bool' &&
      typeof dataToSerialize === 'object' &&
      dataToSerialize !== null &&
      'error' in (dataToSerialize as Record<string, unknown>)
    ) {
      dataToSerialize = { error: Boolean((dataToSerialize as any).error) };
    }

    try {
      return borsh.serialize(borshSchema as Schema, dataToSerialize);
    } catch (error) {
      // Emit schema/payload for debugging serialization issues
      console.error(
        '[OutputSerializer] Borsh serialization failed',
        {
          schema: borshSchema,
          payload: dataToSerialize,
        },
        error
      );
      throw error;
    }
  }

  private static async serializeAbi(
    output: TransactionOutputData,
    schema: Buffer | number[]
  ): Promise<Uint8Array> {
    const schemaStr = this.getSchemaString(schema);
    const parsedSchema = JSON.parse(schemaStr) as AbiSchemaField[];

    let dataToEncode: TransactionOutputData = output;
    if (output.isFunctionCall === false) {
      dataToEncode = this.createAbiData(parsedSchema);
    }

    const values = parsedSchema.map((field) => {
      if (dataToEncode[field.name] === undefined) {
        throw new Error(`Missing required field '${field.name}' in output`);
      }
      return dataToEncode[field.name];
    });

    const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
      parsedSchema.map((s) => s.type),
      values
    );

    return ethers.getBytes(encoded);
  }

  private static getSchemaString(schema: Buffer | number[]): string {
    return typeof schema === 'string'
      ? schema
      : new TextDecoder().decode(new Uint8Array(schema));
  }

  private static createBorshData(
    borshSchema: BorshSchema,
    fallback?: TransactionOutputData
  ): TransactionOutputData {
    const struct = borshSchema.struct;
    if (!struct) {
      return { success: true };
    }

    const obj: TransactionOutputData = {};
    for (const [key, type] of Object.entries(struct)) {
      if (fallback && key in fallback) {
        obj[key] = fallback[key];
        continue;
      }

      switch (type) {
        case 'bool':
          obj[key] = true;
          break;
        case 'string':
          obj[key] = 'non_function_call_success';
          break;
        case 'u8':
        case 'u16':
        case 'u32':
        case 'u64':
        case 'u128':
        case 'i8':
        case 'i16':
        case 'i32':
        case 'i64':
        case 'i128':
          obj[key] = 0;
          break;
        default:
          // Default seed with null for unrecognized types; caller should override.
          obj[key] = null;
      }
    }
    return obj;
  }

  private static createAbiData(
    schema: AbiSchemaField[]
  ): TransactionOutputData {
    const data: TransactionOutputData = {};
    schema.forEach((field) => {
      if (field.type === 'string') {
        data[field.name] = 'non_function_call_success';
      } else if (field.type === 'bool') {
        data[field.name] = true;
      } else {
        throw new Error(
          `Cannot serialize non-function call success as type ${field.type}`
        );
      }
    });
    return data;
  }
}
