
# liquify-scrypto

# Liquify

Liquify is a decentralized unstaking protocol built on the Radix network that enables users to instantly unstake their native Radix Validator LSUs (Liquid Staking Units) through a decentralized liquidity pool. The protocol matches LSU sellers with XRD liquidity providers who receive LSUs or unstake NFTs in return for providing instant liquidity.

## Overview

The Liquify protocol solves the unstaking delay problem in the Radix network by creating a marketplace where:

1. Liquidity providers can deposit XRD and specify a discount rate they're willing to accept for buying LSUs
2. Users holding LSUs can instantly "unstake" by selling their LSUs to the liquidity pool at the best available rates
3. Liquidity providers can either collect LSUs or have them automatically unstaked on their behalf

### Key Features

- Instant unstaking of LSUs through liquidity pool
- Automated discount-based order matching system
- Support for native Radix validator LSUs
- Configurable auto-unstaking option for liquidity providers
- Efficient order filling with AVL tree data structures
- Comprehensive fee system for platform sustainability

## Core Components

### LiquidityDetails

The primary data structure tracking liquidity provision contains:

- Key image URL
- Liquidity status (Open/Cancelled/Closed)
- Total XRD amount
- Discount rate
- Remaining XRD
- Number of fills to collect
- Fill percentage
- Auto-unstake preference

### Key Methods

#### For LSU Holders

1. `liquify_unstake`
   - Instantly converts LSUs to XRD using available liquidity
   - Automatically matches with best available rates
   - Returns XRD and any remaining unmatched LSUs

2. `liquify_unstake_off_ledger`
   - Similar to liquify_unstake but with specified liquidity receipts
   - Allows for more controlled unstaking process

#### For Liquidity Providers

1. `add_liquidity`
   - Deposit XRD into the liquidity pool
   - Specify discount rate for LSU purchases
   - Receive liquidity receipt NFT
   - Set auto-unstake preference

2. `remove_liquidity`
   - Withdraw deposited XRD liquidity
   - Only available for unfilled or partially filled orders
   - Returns remaining XRD and receipt NFTs

3. `collect_fills`
   - Collect LSUs or unstake NFTs from filled orders
   - Returns vector of assets and updated receipt NFTs

4. `burn_closed_receipts`
   - Optional burning of closed liquidity receipt NFTs

## Technical Implementation

### Data Structures

1. `CombinedKey`
   - Combines liquidity ID and discount key into single u128
   - Used for efficient order matching and tracking

2. `AvlTree`
   - Maintains ordered list of buy orders
   - Enables efficient order matching and fill tracking

### Status Enums

1. `LiquidityStatus`
   - Open: Active and accepting fills
   - Cancelled: Manually cancelled by provider
   - Closed: Fully filled and collected

2. `FillStatus`
   - Unfilled: No fills yet
   - Filled: Completely filled
   - PartiallyFilled: Partially filled

## Protocol Parameters

- `minimum_liquidity`: Minimum XRD required for liquidity provision
- `max_liquidity_iter`: Maximum number of iterations for unstaking (28-29)
- `max_fills_to_collect`: Maximum fills collectible in single transaction (85)
- `platform_fee`: Configurable fee percentage
- Discount steps: 0.025% increments from 0-5%



Liquify Interface Component
The Liquify Interface component serves as a stable integration point for the protocol, providing:
Purpose

Maintains a persistent component address for on-ledger integrations
Enables seamless protocol upgrades without breaking existing integrations
Provides a standardized interface for external callers

Implementation

Implements pass-through methods matching all public Liquify methods
Maintains identical method signatures for consistency
Forwards all arguments and returns directly to/from the core Liquify component





iterations:

off ledger, no interface, auto unstake = true, max = 29
off ledger, no interface, auto unstake = false, max = 42

