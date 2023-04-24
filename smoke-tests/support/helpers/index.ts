import { ApiPromise } from '@polkadot/api'
import { Codec } from '@polkadot/types/types'
import { Keyring, createTestKeyring } from '@polkadot/keyring'
import { expect } from 'vitest'

type CodecOrArray = Codec | Codec[]

const processCodecOrArray = (codec: CodecOrArray, fn: (c: Codec) => any) =>
  Array.isArray(codec) ? codec.map(fn) : fn(codec)

const toHuman = (codec: CodecOrArray) => processCodecOrArray(codec, (c) => c?.toHuman?.() ?? c)
const toHex = (codec: CodecOrArray) => processCodecOrArray(codec, (c) => c?.toHex?.() ?? c)
const toJson = (codec: CodecOrArray) => processCodecOrArray(codec, (c) => c?.toJSON?.() ?? c)

export type EventFilter = string | { method: string; section: string }

export type RedactOptions = {
  number?: boolean | number // precision
  hash?: boolean // 32 byte hex
  hex?: boolean // any hex with 0x prefix
  address?: boolean // base58 address
}

export class Checker {
  readonly #value: any
  readonly #pipeline: Array<(value: any) => any> = []

  #format: 'human' | 'hex' | 'json' = 'json'
  #message: string | undefined
  #redactOptions: RedactOptions | undefined

  constructor(value: any, message?: string) {
    this.#value = value
    this.#message = message
  }

  toHuman() {
    this.#format = 'human'
    return this
  }

  toHex() {
    this.#format = 'hex'
    return this
  }

  toJson() {
    this.#format = 'json'
    return this
  }

  message(message: string) {
    this.#message = message
    return this
  }

  filterEvents(...filters: EventFilter[]) {
    this.toHuman()
    this.#pipeline.push((value) => {
      let data = value.map(({ event: { index: _, ...event } }: any) => event)
      if (filters.length > 0) {
        data = data.filter((evt: any) => {
          return filters.some((filter) => {
            if (typeof filter === 'string') {
              return evt.section === filter
            } else if ('method' in filter) {
              const { section, method } = filter
              return evt.section === section && evt.method === method
            }
          })
        })
      }
      return data
    })
    return this
  }

  redact(options: RedactOptions = { number: 2, hash: true }) {
    this.#redactOptions = {
      ...this.#redactOptions,
      ...options,
    }
    return this
  }

  #redact(value: any) {
    if (!this.#redactOptions) {
      return value
    }

    const redactNumber = this.#redactOptions.number === true || typeof this.#redactOptions.number === 'number'
    const precision = redactNumber
      ? typeof this.#redactOptions.number === 'number'
        ? this.#redactOptions.number
        : 0
      : 0
    const redactHash = this.#redactOptions.hash === true
    const redactHex = this.#redactOptions.hex === true
    const redactAddress = this.#redactOptions.address === true

    const processNumber = (value: number) => {
      if (precision > 0) {
        const rounded = parseFloat(value.toPrecision(precision))
        if (rounded === value) {
          return rounded
        }
        return `(rounded ${rounded})`
      }
      return '(number)'
    }

    const process = (obj: any): any => {
      if (obj == null) {
        return obj
      }
      if (Array.isArray(obj)) {
        return obj.map(process)
      }
      if (redactNumber && typeof obj === 'number') {
        return processNumber(obj)
      }
      if (typeof obj === 'string') {
        if (redactNumber && obj.match(/0x000000[0-9a-f]{26}/)) {
          // this is very likely u128 encoded in hex
          const num = parseInt(obj)
          return processNumber(num)
        }
        if (redactHash && obj.match(/0x[0-9a-f]{64}/)) {
          return '(hash)'
        }
        if (redactHex && obj.match(/0x[0-9a-f]+/)) {
          return '(hex)'
        }
        if (redactAddress && obj.match(/^[1-9A-HJ-NP-Za-km-z]{46,48}$/)) {
          return '(address)'
        }
        if (redactNumber && obj.match(/^-?[\d,]+$/)) {
          const num = parseInt(obj.replace(/,/g, ''))
          return processNumber(num)
        }
        return obj
      }
      if (typeof obj === 'object') {
        return Object.fromEntries(Object.entries(obj).map(([k, v]) => [k, process(v)]))
      }
      return obj
    }

    return process(value)
  }

  map(fn: (value: any) => any) {
    this.#pipeline.push(fn)
    return this
  }

  pipe(fn?: (value: Checker) => Checker) {
    return fn ? fn(this) : this
  }

  async value() {
    let value = await this.#value

    switch (this.#format) {
      case 'human':
        value = toHuman(value)
        break
      case 'hex':
        value = toHex(value)
        break
      case 'json':
        value = toJson(value)
        break
    }

    for (const fn of this.#pipeline) {
      value = await fn(value)
    }

    value = this.#redact(value)

    return value
  }

  async toMatchSnapshot(msg?: string) {
    return expect(await this.value()).toMatchSnapshot(msg ?? this.#message)
  }
}

export const check = (value: any, msg?: string) => {
  if (value instanceof Checker) {
    if (msg) {
      return value.message(msg)
    }
    return value
  }
  return new Checker(value, msg)
}

type Api = { api: ApiPromise }

export const checkEvents = ({ events }: { events: Promise<Codec[] | Codec> }, ...filters: EventFilter[]) =>
  check(events, 'events')
    .filterEvents(...filters)
    .redact()

export const checkSystemEvents = ({ api }: Api, ...filters: EventFilter[]) =>
  check(api.query.system.events(), 'system events')
    .filterEvents(...filters)
    .redact()

export const checkUmp = ({ api }: Api) =>
  check(api.query.parachainSystem.upwardMessages(), 'ump').map((value) =>
    api.createType('Vec<XcmVersionedXcm>', value).toJSON()
  )

export const checkHrmp = ({ api }: Api) =>
  check(api.query.parachainSystem.hrmpOutboundMessages(), 'hrmp').map((value) =>
    (value as any[]).map(({ recipient, data }) => ({
      data: api.createType('(XcmpMessageFormat, XcmVersionedXcm)', data),
      recipient,
    }))
  )

export const testingPairs = () => {
  const keyringEth = createTestKeyring({ type: 'ethereum' })
  const keyring = new Keyring({ type: 'ed25519' }) // can't use sr25519 because it is non-deterministic
  return {
    alice: keyring.addFromUri('//Alice'),
    bob: keyring.addFromUri('//Bob'),
    charlie: keyring.addFromUri('//Charlie'),
    dave: keyring.addFromUri('//Dave'),
    eve: keyring.addFromUri('//Eve'),

    alith: keyringEth.getPair('0xf24FF3a9CF04c71Dbc94D0b566f7A27B94566cac'),
    baltathar: keyringEth.getPair('0x3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0'),
    charleth: keyringEth.getPair('0x798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc'),
    dorothy: keyringEth.getPair('0x773539d4Ac0e786233D90A233654ccEE26a613D9'),
    ethan: keyringEth.getPair('0xFf64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB'),

    keyring,
    keyringEth,
  }
}
