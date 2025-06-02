# sbor-ez-mode

The simple way to parse Programmatic JSON into nice JavaScript objects with an api inspired by Zod.

## Installation

```sh
npm install @calamari-radix/sbor-ez-mode
```

## Warning: The APIs in this repository may not be stable and could be changed at any time.

## Usage

## Parsing ProgrammaticScryptoSborValue

The Radix network uses a custom data serialization standard called Scrypto SBOR for storing on-ledger state. This data format can be represented and consumed as JSON in a special representation called Scrypto SBOR Programmatic JSON. While the Babylon Gateway SDK has good types for this programmatic JSON, it is quite difficult and time-consuming to parse the values coming from ledger into workable JS/TS values.

This package contains a library inspired by the popular validation library Zod, that makes it easy to write TypeScript schemas for your Scrypto types and parse them into nice javascript objects, arrays, maps etc in one go.

### A note on error handling

This package uses [neverthrow](https://github.com/supermacro/neverthrow)'s `Result` type for error handling. This means that the result of a safeParse operation will always be a `Result` object, which can be either `Ok` or `Err`. You can check if the result is `Ok` or `Err` and then access the value or error accordingly. For more information on how to use the Result type, please refer to the neverthrow documentation.

### A simple example:

```ts
// s (for sbor) is the main object you'll have to import to create schemas and parse
import s from '@calamari-radix/sbor-ez-mode';

const swapEventSchema = s.struct({
    input_address: s.address(),
    input_amount: s.decimal(),
    output_address: s.address(),
    output_amount: s.decimal(),
    is_success: s.bool(),
});
```

This schema represents the following Rust struct:

```rust
struct SwapEvent {
    input_address: ResourceAddress,
    input_amount: Decimal,
    output_address: ResourceAddress,
    output_amount: Decimal,
    is_success: bool
}
```

The programmatic json might look like this:
Don't be fooled! The `Kind` is `Tuple`, but this is surely a struct!
Scrypto SBOR does not have a Struct Kind natively, it is represented by Tuple.

```json
{
    "fields": [
        {
            "value": "resource_rdx1t5pyvlaas0ljxy0wytm5gvyamyv896m69njqdmm2stukr3xexc2up9",
            "kind": "Reference",
            "type_name": "ResourceAddress",
            "field_name": "input_address"
        },
        {
            "value": "0.003427947474666592",
            "kind": "Decimal",
            "field_name": "input_amount"
        },
        {
            "value": "resource_rdx1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxradxrd",
            "kind": "Reference",
            "type_name": "ResourceAddress",
            "field_name": "output_address"
        },
        {
            "value": "522.23800528105807128",
            "kind": "Decimal",
            "field_name": "output_amount"
        },
        {
            "field_name": "is_success",
            "kind": "Bool",
            "value": true
        }
    ],
    "kind": "Tuple",
    "type_name": "SwapEvent"
}
```

Now we can parse this programmatic JSON into our nice schema:

```ts
// The return type is `neverthrow`'s `Result` type, which you can nicely handle
// in a lot of ways.
const result = swapEventSchema.safeParse(example);

if (result.isOk()) {
    console.log(result.value);
}
// {
//     input_address: 'resource_rdx1t5py...',
//     input_amount: '0.003427947474666592',
//     output_address: 'resource_rdx1tknxxxx...',
//     output_amount: '522.23800528105807128',
//     is_success: true,
// }
```

So now we managed to go from this huge JSON representation to an easy to use, pretty object representation.

### Parsing arrays

Rust type:

```rust
Vec<u32>
```

```ts
const example: ProgrammaticScryptoSborValue = {
    kind: 'Array',
    field_name: 'nft_ids',
    type_name: 'Array',
    element_kind: 'NonFungibleLocalId',
    elements: [
        {
            kind: 'NonFungibleLocalId',
            value: '#1#',
        },
        {
            kind: 'NonFungibleLocalId',
            value: '#2#',
        },
        {
            kind: 'NonFungibleLocalId',
            value: '#3#',
        },
    ],
};
const schema = s.array(s.nonFungibleLocalId());
const result = schema.safeParse(example);

if (result.isOk()) {
    console.log(result.value);
}

// ['#1#', '#2#', '#3#'];
```

### Parsing maps

Rust type:

```rust
HashMap<String, String>
```

```ts
const example: ProgrammaticScryptoSborValue = {
    kind: 'Map',
    field_name: 'map',
    key_kind: 'String',
    value_kind: 'String',
    entries: [
        {
            key: {
                kind: 'String',
                value: 'boinoing',
            },
            value: {
                kind: 'String',
                value: 'boobies',
            },
        },
        {
            key: {
                kind: 'String',
                value: 'impostor',
            },
            value: {
                kind: 'String',
                value: 'amogus',
            },
        },
    ],
};
```

```ts
const schema = s.map({
    key: s.string(),
    value: s.string(),
});

const result = schema.safeParse(example);

if (result.isOk()) {
    console.log(result.value);
}

// Map(2) { 'boinoing' => 'boobies', 'impostor' => 'amogus' }
```

### Parsing a tuple

Rust type:

```rust
(String, u32)
```

```ts
const example: ProgrammaticScryptoSborValue = {
    kind: 'Tuple',
    field_name: 'tuple',
    type_name: 'tuple',
    fields: [
        {
            kind: 'String',
            value: 'hello',
        },
        {
            kind: 'U32',
            value: '5',
        },
    ],
};

const schema = s.tuple([s.string(), s.number()]);
const result = schema.safeParse(example);

if (result.isOk()) {
    console.log(result.value);
}

// ['hello', 5];
```

## Parsing enums

The corresponding Rust type:

```rust
enum MyEnum {
    NonFungible {
        ids: Vec<NonFungibleLocalId>,
        resource_address: ResourceAddress,
    },
    Fungible(Decimal),
}
```

To parse enums, you will have to provide the `enum` schema constructor with an array of variant definitions. The schema passed to `schema` _must_ be either a struct or a tuple schema, but of course it can then contain any other schema. The result will have the nice type inference you would expect, meeaning you can do type narrowing on the result to figure out which variant was parsed and act accordingly.

```ts
const myEnumSchema = s.enum([
    {
        variant: 'NonFungible',
        schema: s.struct({
            ids: s.array(s.nonFungibleLocalId()),
            resource_address: s.address(),
        }),
    },
    { variant: 'Fungible', schema: s.tuple([s.decimal()]) },
]);
```

In the case of a `NonFungible` variant, the programmatic JSON might look like this:

```ts
const example: ProgrammaticScryptoSborValue = {
    kind: 'Enum',
    variant_id: '0',
    variant_name: 'NonFungible',
    fields: [
        {
            kind: 'Array',
            field_name: 'ids',
            values: [
                {
                    kind: 'NonFungibleLocalId',
                    value: '#1#',
                },
            ],
        },
        {
            kind: 'Reference',
            field_name: 'resource_address',
            value: 'resource_rdx1t5pyvlaas0ljxy0wytm5gvyamyv896m69njqdmm2stukr3xexc2up9',
        },
    ],
};
```

Now we can parse this programmatic JSON into our nice schema:

```ts
const result = myEnumSchema.safeParse(example);

if (result.isOk()) {
    console.log(result.value);
}

// {
//     variant: 'NonFungible',
//     value: {
//         ids: ['#1#'],
//         resource_address: 'resource_rdx1t5pyvlaas0ljxy0wytm5gvyamyv896m69njqdmm2stukr3xexc2up9',
//     },
// }
```

### Using the option utility enum

The `option` utility enum is a special enum schema that can be used to represent optional values. It has two variants: `Some` and `None`.

The corresponding Rust type:

```rust
Option<u32>
```

Example of parsing a `None`:

```ts
const example: ProgrammaticScryptoSborValue = {
    kind: 'Enum',
    variant_id: '0',
    variant_name: 'None',
    fields: [],
};

const schema = s.option(s.string());

const result = schema.safeParse(example);

if (result.isOk()) {
    console.log(result.value);
}

// { variant: 'None' }
```

Example of parsing a `Some`:

```ts
const example: ProgrammaticScryptoSborValue = {
    kind: 'Enum',
    variant_id: '1',
    variant_name: 'Some',
    fields: [
        {
            kind: 'Tuple',
            fields: [
                {
                    kind: 'Bool',
                    value: true,
                },
            ],
        },
    ],
};

const schema = s.option(s.bool());

const result = schema.safeParse(example);

if (result.isOk()) {
    console.log(result.value);
}

// { variant: 'Some', value: true }
```
