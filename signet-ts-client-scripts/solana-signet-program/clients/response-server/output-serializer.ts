import * as borsh from 'borsh'
import { ethers } from 'ethers'

export class OutputSerializer {
  static async serialize(
    output: any,
    format: number,
    schema: any
  ): Promise<Uint8Array> {
    console.log('\n📦 Serializing Output')
    console.log(`  📋 Format: ${format === 0 ? 'Borsh' : 'AbiJson'}`)
    console.log('  📊 Output:', output)

    if (format === 0) {
      return this.serializeBorsh(output, schema)
    } else if (format === 1) {
      return this.serializeAbi(output, schema)
    }

    throw new Error(`Unsupported serialization format: ${format}`)
  }

  private static async serializeBorsh(
    output: any,
    schema: any
  ): Promise<Uint8Array> {
    const schemaStr = this.getSchemaString(schema)
    console.log('schemaStr', {
      schemaStr,
    })
    const borshSchema = JSON.parse(schemaStr)

    console.log('borshSchema', {
      borshSchema,
    })

    let dataToSerialize = output
    if (output.isFunctionCall === false) {
      dataToSerialize = this.createBorshData(borshSchema)
    }

    if (typeof borshSchema === 'string' && borshSchema === 'bool') {
      if (typeof dataToSerialize === 'object' && dataToSerialize !== null) {
        dataToSerialize =
          dataToSerialize.success ??
          dataToSerialize[Object.keys(dataToSerialize)[0]]
      }
    }

    console.log('dataToSerialize', {
      dataToSerialize,
    })

    const serialized = borsh.serialize(borshSchema, dataToSerialize)
    console.log(`  📏 Serialized length: ${serialized.length} bytes`)

    return serialized
  }

  private static async serializeAbi(
    output: any,
    schema: any
  ): Promise<Uint8Array> {
    const schemaStr = this.getSchemaString(schema)
    const parsedSchema = JSON.parse(schemaStr)

    let dataToEncode = output
    if (output.isFunctionCall === false) {
      dataToEncode = this.createAbiData(parsedSchema)
    }

    const values = parsedSchema.map((field: any) => {
      if (dataToEncode[field.name] === undefined) {
        throw new Error(`Missing required field '${field.name}' in output`)
      }
      return dataToEncode[field.name]
    })

    const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
      parsedSchema.map((s: any) => s.type),
      values
    )

    return ethers.getBytes(encoded)
  }

  private static getSchemaString(schema: any): string {
    return typeof schema === 'string'
      ? schema
      : new TextDecoder().decode(new Uint8Array(schema))
  }

  private static createBorshData(borshSchema: any): any {
    if (borshSchema.struct && borshSchema.struct.message === 'string') {
      return { message: 'non_function_call_success' }
    } else if (borshSchema.struct && borshSchema.struct.success === 'bool') {
      return { success: true }
    }
    return { success: true }
  }

  private static createAbiData(schema: any[]): any {
    const data: any = {}
    schema.forEach((field: any) => {
      if (field.type === 'string') {
        data[field.name] = 'non_function_call_success'
      } else if (field.type === 'bool') {
        data[field.name] = true
      } else {
        throw new Error(
          `Cannot serialize non-function call success as type ${field.type}`
        )
      }
    })
    return data
  }
}
