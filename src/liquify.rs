use scrypto::prelude::*;
use scrypto_avltree::AvlTree;

#[derive(ScryptoSbor, NonFungibleData, Debug)]
pub struct UnstakeData {
    pub name: String,
    pub claim_epoch: Epoch,
    pub claim_amount: Decimal,
}

#[derive(ScryptoSbor, Debug, Clone)]
pub struct ReceiptDetailData {
    pub receipt_id: NonFungibleLocalId,
    pub discount: Decimal,
    pub auto_unstake: bool,
    pub auto_refill: bool,
    pub refill_threshold: Decimal,
    pub xrd_liquidity_available: Decimal,
    pub xrd_liquidity_filled: Decimal,
    pub liquidity_ahead: Decimal,
    pub fills_to_collect: u64,
    pub last_added_epoch: u32,
    pub claimable_xrd: Decimal,
    pub total_stake_claim_value: Decimal,
    pub total_lsu_redemption_value: Decimal,
}

#[derive(ScryptoSbor, Debug, Clone)]
pub struct AutomationReadyReceipt {
    pub receipt_id: NonFungibleLocalId,
    pub discount: Decimal,
    pub fills_to_collect: u64,
    pub claimable_xrd: Decimal,
    pub refill_threshold: Decimal,
    pub automation_fee: Decimal,
}

#[derive(NonFungibleData, ScryptoSbor, PartialEq, Debug, Clone)]
pub struct LiquidityReceipt {
    key_image_url: Url,
    discount: Decimal,
    auto_unstake: bool,
    #[mutable]
    auto_refill: bool,
    #[mutable]
    refill_threshold: Decimal,
    #[mutable]
    automation_fee: Decimal,
}

#[derive(ScryptoSbor, PartialEq, Debug, Clone)]
pub struct LiquidityData {
    pub xrd_liquidity_filled: Decimal,
    pub xrd_liquidity_available: Decimal,
    pub fills_to_collect: u64,
    pub last_added_epoch: u32,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct LiquidityAddedEvent {
    receipt_id: NonFungibleLocalId,
    xrd_amount: Decimal,
    discount: Decimal,
    auto_unstake: bool,
    auto_refill: bool,
    refill_threshold: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct LiquidityIncreasedEvent {
    receipt_id: NonFungibleLocalId,
    additional_xrd: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct OrderFillEvent {
    receipt_id: NonFungibleLocalId,
    lsu_amount: Decimal,
    xrd_amount: Decimal,
    discount: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct LiquifyUnstakeEvent {
    lsu_resource: ResourceAddress,
    lsu_amount: Decimal,
    xrd_received: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct LiquidityRemovedEvent {
    receipt_id: NonFungibleLocalId,
    xrd_amount: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct CollectFillsEvent {
    receipt_id: NonFungibleLocalId,
    fills_collected: u64,
    lsus_collected: Vec<(Decimal, ResourceAddress)>,
    stake_claim_nfts_collected: Vec<NonFungibleGlobalId>,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct LiquidityCycledEvent {
    receipt_id: NonFungibleLocalId,
    xrd_amount_cycled: Decimal,
    automation_fee: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct AutoRefillStatusUpdatedEvent {
    receipt_id: NonFungibleLocalId,
    auto_refill: bool,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct RefillThresholdUpdatedEvent {
    receipt_id: NonFungibleLocalId,
    refill_threshold: Decimal,
}

#[derive(ScryptoSbor, ScryptoEvent)]
struct AutomationFeeUpdatedEvent {
    receipt_id: NonFungibleLocalId,
    automation_fee: Decimal,
}

pub struct BuyListKey;

impl BuyListKey {
    pub fn new(discount_basis_points: u16, auto_unstake: bool, position: u64, receipt_id: u32) -> u128 {
        // Pack: discount (16 bits) | auto_unstake (16 bits) | position (64 bits) | receipt_id (32 bits) = 128 bits
        let auto_unstake_flag = if auto_unstake { 1u16 } else { 0u16 };
        
        ((discount_basis_points as u128) << 112) |  // Top 16 bits
        ((auto_unstake_flag as u128) << 96) |       // Next 16 bits  
        ((position as u128) << 32) |                // Next 64 bits
        (receipt_id as u128)                        // Bottom 32 bits
    }
    
    pub fn extract_auto_unstake(key: u128) -> bool {
        ((key >> 96) & 0xFFFF) == 1
    }
}

pub struct OrderFillKey;

impl OrderFillKey {
    pub fn new(receipt_id: u64, fill_number: u64) -> u128 {
        // Pack: receipt_id (64 bits) | fill_number (64 bits) = 128 bits
        ((receipt_id as u128) << 64) | (fill_number as u128)
    }
}

#[derive(ScryptoSbor, PartialEq, Debug, Clone)]
pub enum UnstakeNFTOrLSU {
    UnstakeNFT(UnstakeNFTData),    
    LSU(LSUData)
}

#[derive(ScryptoSbor, PartialEq, Debug, Clone)]
pub struct UnstakeNFTData {
    resource_address: ResourceAddress,
    id: NonFungibleLocalId
}

#[derive(ScryptoSbor, PartialEq, Debug, Clone)]
pub struct LSUData {
    resource_address: ResourceAddress,
    amount: Decimal
}

#[blueprint]
#[types(Decimal, ResourceAddress, LiquidityReceipt, LiquidityData, NonFungibleLocalId, NonFungibleGlobalId, ComponentAddress, i64, u64, Vault)]
#[events(
    LiquifyUnstakeEvent,
    OrderFillEvent, 
    LiquidityAddedEvent,
    LiquidityIncreasedEvent,
    LiquidityRemovedEvent,
    CollectFillsEvent,
    LiquidityCycledEvent,
    AutoRefillStatusUpdatedEvent,
    RefillThresholdUpdatedEvent,
    AutomationFeeUpdatedEvent,

)]
mod liquify_module {
    enable_method_auth! {
        roles {
            owner => updatable_by: [];
        },
        methods {
            
            add_liquidity => PUBLIC;
            increase_liquidity => PUBLIC;
            remove_liquidity => PUBLIC;
            liquify_unstake => PUBLIC;
            liquify_unstake_off_ledger => PUBLIC;
            collect_fills => PUBLIC;
            update_auto_refill_status => PUBLIC;
            update_refill_threshold => PUBLIC;
            update_automation_fee => PUBLIC;
            cycle_liquidity => PUBLIC;
            calculate_claimable_xrd_and_ordered_list => PUBLIC;

            get_claimable_xrd => PUBLIC;
            get_raw_buy_list_range => PUBLIC;
            get_automation_ready_receipts => PUBLIC;
            get_receipt_detail => PUBLIC;
            get_active_liquidity_positions => PUBLIC;

            set_component_status => restrict_to: [owner];
            set_platform_fee => restrict_to: [owner];
            set_minimum_liquidity => restrict_to: [owner];
            set_receipt_image_url => restrict_to: [owner];
            set_minimum_refill_threshold => restrict_to: [owner];
            set_max_fills_per_cycle => restrict_to: [owner];
            set_unstake_value_range => restrict_to: [owner];
            set_small_order_threshold => restrict_to: [owner];
            collect_platform_fees => restrict_to: [owner];
        }
    }

    struct Liquify {
        liquify_owner_badge: ResourceAddress,
        xrd_liquidity: Vault,
        liquidity_receipt: NonFungibleResourceManager,
        liquidity_receipt_counter: u64,
        buy_list: AvlTree<u128, NonFungibleGlobalId>,
        order_fill_tree: AvlTree<u128, UnstakeNFTOrLSU>,
        component_vaults: KeyValueStore<ResourceAddress, Vault>,
        liquidity_data: KeyValueStore<NonFungibleGlobalId, LiquidityData>,
        total_xrd_volume: Decimal,
        total_xrd_locked: Decimal,
        component_status: bool,
        order_fill_counter: u64,
        avl_position_counter: u64,  
        liquidity_index: Vec<Decimal>,
        discounts: Vec<Decimal>,
        platform_fee: Decimal,
        fee_vault: Vault,
        minimum_liquidity: Decimal,
        minimum_refill_threshold: Decimal,
        minimum_unstake_value: Decimal,
        maximum_unstake_value: Decimal,
        receipt_image_url: Url,
        automation_fee: Decimal,
        automated_liquidity: KeyValueStore<u64, NonFungibleGlobalId>,
        automated_liquidity_index: u64,
        max_fills_per_cycle: u64,
        small_order_threshold: Decimal,
    }

    impl Liquify {

        /// Instantiates a new Liquify component for LSU liquidity provision.
        /// 
        /// This method creates a new instance of the Liquify component which facilitates instant unstaking of Radix
        /// network LSUs by matching unstakers with liquidity providers. The component starts in a disabled state
        /// and must be enabled by the owner before accepting liquidity. It initializes all necessary data structures
        /// including the AVL tree for order matching, key-value stores for tracking liquidity positions, and creates
        /// the owner badge and liquidity receipt NFT resource. The component supports discounts from 0% to 5% in
        /// increments of 0.025%.
        /// 
        /// # Arguments
        /// * None
        ///
        /// # Returns
        /// * A tuple containing:
        ///   - `Global<Liquify>`: The instantiated Liquify component
        ///   - `Bucket`: The owner badge bucket containing exactly 1 owner badge
        pub fn instantiate_liquify() -> (Global<Liquify>, Bucket) {

            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Liquify::blueprint_id());

            let liquify_owner_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata!(
                    init {
                        "name" => "Liquify Owner Badge".to_string(), locked;
                        "icon_url" => Url::of("https://bafybeif5tjpcgjgfo2lt6pp3qnz5s7mdpejfhgkracs7hzreoeg3bw3wae.ipfs.w3s.link/liquify_icon.png"), updatable;
                    }
                ))
                .divisibility(DIVISIBILITY_NONE)
                .mint_initial_supply(1)
                .into();

            let tags = vec!["Liquify", "Liquidity", "LSU"];
            
            let liquidity_receipt = ResourceBuilder::new_integer_non_fungible::<LiquidityReceipt>(OwnerRole::Fixed(
                rule!(require_any_of(vec![global_caller(component_address), ResourceOrNonFungible::Resource(liquify_owner_badge.resource_address())]))))
                .metadata(metadata!(
                    init {
                        "name" => "Liquify Liquidity Receipt".to_owned(), updatable;
                        "description" => "Receipt for providing liquidity on the Liquify platform".to_string(), updatable;
                        "icon_url" => Url::of("https://bafybeif5tjpcgjgfo2lt6pp3qnz5s7mdpejfhgkracs7hzreoeg3bw3wae.ipfs.w3s.link/liquify_icon.png"), updatable;
                        "tags" => tags.clone(), updatable;
                    }
                ))
                .mint_roles(mint_roles!{
                    minter => rule!(require(global_caller(component_address)));
                    minter_updater => rule!(deny_all); 
                })
                .burn_roles(burn_roles! {
                    burner => rule!(require(global_caller(component_address)));
                    burner_updater => rule!(deny_all);
                })
                .non_fungible_data_update_roles(non_fungible_data_update_roles! {
                    non_fungible_data_updater => rule!(require(global_caller(component_address)));
                    non_fungible_data_updater_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();
            
            let mut liquidity_index: Vec<Decimal> = Vec::new();
            let mut discounts: Vec<Decimal> = Vec::new();
            let step: Decimal = dec!(0.00025);

            for i in 0..=200 {
                let discount = step * Decimal::from(i);
                liquidity_index.push(Decimal::ZERO);
                discounts.push(discount);
            }

            let liquify_component = Liquify {
                liquify_owner_badge: liquify_owner_badge.resource_address(),
                xrd_liquidity: Vault::new(XRD),
                liquidity_receipt,
                liquidity_receipt_counter: 1,
                avl_position_counter: 1,
                buy_list: AvlTree::new(),
                order_fill_tree: AvlTree::new(),
                component_vaults: KeyValueStore::new_with_registered_type(),
                liquidity_data: KeyValueStore::new_with_registered_type(),
                liquidity_index,
                discounts,
                total_xrd_volume: Decimal::ZERO,
                total_xrd_locked: Decimal::ZERO,
                component_status: false, 
                order_fill_counter: 1,
                platform_fee: dec!(0.0005), // 0.0005 = 0.05% platform fee
                fee_vault: Vault::new(XRD),
                minimum_liquidity: dec!(10000),
                minimum_refill_threshold: dec!(10000),
                minimum_unstake_value: dec!(0),
                maximum_unstake_value: dec!(10000000),
                receipt_image_url: Url::of("https://bafybeib7cokm27lwwkunaibn7hczijn3ztkypbzttmt7hymaov44s5e5sm.ipfs.w3s.link/liquify2.png"),
                automation_fee: dec!(5),
                automated_liquidity: KeyValueStore::new_with_registered_type(),
                automated_liquidity_index: 1,
                max_fills_per_cycle: 50,
                small_order_threshold: dec!(1000), 
            }
            .instantiate()
            .prepare_to_globalize(
                OwnerRole::Fixed(
                    rule!(require(liquify_owner_badge.resource_address())
                )
            ))
            .roles(
                roles!(
                    owner => rule!(require(liquify_owner_badge.resource_address()));
                )
            )
            .with_address(address_reservation)
            .metadata(metadata!(
                init {
                    "name" => "Liquify".to_string(), updatable;
                    "description" => "Liquify Unstaking platform for native Radix liquid stake units.".to_string(), updatable;
                }
            ))
            .enable_component_royalties(component_royalties! {
                init {
                    add_liquidity => Free, updatable;
                    increase_liquidity => Free, updatable;
                    remove_liquidity => Free, updatable;
                    liquify_unstake => Free, updatable;
                    liquify_unstake_off_ledger => Free, updatable;
                    collect_fills => Free, updatable;
                    update_auto_refill_status => Free, updatable;
                    update_refill_threshold => Free, updatable;
                    update_automation_fee => Free, updatable;
                    cycle_liquidity => Free, updatable;
                    calculate_claimable_xrd_and_ordered_list => Free, updatable;
                    get_claimable_xrd => Free, updatable;
                    set_component_status => Free, updatable;
                    set_platform_fee => Free, updatable;
                    set_max_fills_per_cycle => Free, updatable;
                    collect_platform_fees => Free, updatable;
                    set_minimum_liquidity => Free, updatable;
                    set_receipt_image_url => Free, updatable;
                    get_raw_buy_list_range => Free, updatable;
                    get_active_liquidity_positions => Free, updatable;
                    set_minimum_refill_threshold => Free, updatable;
                    set_unstake_value_range => Free, updatable;
                    set_small_order_threshold => Free, updatable;
                    get_automation_ready_receipts => Free, updatable;
                    get_receipt_detail => Free, updatable;

                }
            })
            .globalize();

            (liquify_component, liquify_owner_badge)
        }

        /// Allows user to deposit XRD liquidity with specified parameters.
        /// 
        /// This method takes a bucket of XRD, a discount of type Decimal that indicates the percentage amount under
        /// the redemption value of an amount of LSUs that a liquidity provider is willing to purchase any LSU, and
        /// a boolean that indicates whether the user wants to automatically unstake any LSUs that are collected. This
        /// method is constrained by the `minimum_liquidity` variable. The user must pass in an amount of XRD that is greater
        /// than or equal to the `minimum_liquidity` which is set to 10,000 XRD by default. This can be adjusted by the owner
        /// of the component in order to maintain an efficient unstaking process. The higher the minimum, the more liquidity
        /// can be processed in a single transaction. Auto refill can only be enabled when auto unstake is also enabled.
        /// 
        /// # Arguments
        /// * `xrd_bucket`: A `Bucket` containing XRD to be deposited as liquidity
        /// * `discount`: A `Decimal` representing the discount percentage the user is willing to use liquidity provided
        /// * `auto_unstake`: A `bool` indicating whether the user wants to automatically unstake any LSUs that are collected
        /// * `auto_refill`: A `bool` indicating whether the user wants to automatically refill liquidity from collected fills
        /// * `refill_threshold`: A `Decimal` representing the minimum XRD amount needed to trigger auto refill
        ///
        /// # Returns
        /// * A `NonFungibleBucket` containing the new liquidity receipt NFT that has been minted to track the liquidity
        pub fn add_liquidity(
            &mut self, 
            xrd_bucket: Bucket, 
            discount: Decimal, 
            auto_unstake: bool, 
            auto_refill: bool, 
            refill_threshold: Decimal,
            automation_fee: Decimal
            ) -> NonFungibleBucket {
            
            assert!(self.component_status == true, "Liquify is not accepting new liquidity at this time.");
            assert!(xrd_bucket.resource_address() == XRD, "Bucket must contain XRD");
            assert!(xrd_bucket.amount() >= self.minimum_liquidity, "This amount is below the minimum liquidity requirement XRD");
            assert!(self.discounts.contains(&discount), "This discount % is not supported");
            
            // ADDED: Validate auto_refill requires auto_unstake
            if auto_refill {
                assert!(auto_unstake, "Auto refill can only be enabled when auto unstake is enabled");
                assert!(refill_threshold >= self.minimum_refill_threshold, "Refill threshold is below required minimum");
            }

            // Convert discount to basis points for the key
            let discount_basis_points: u16 = match (discount * dec!(10000)).checked_floor() {
                Some(val) => match val.to_string().parse::<u16>() {
                    Ok(points) => points,  // Convert to u16
                    Err(_) => panic!("Failed to parse discount basis points")
                },
                None => panic!("Failed to convert discount to basis points")
            };

            // Create buy list key with new structure - now includes auto_unstake
            let receipt_id_u32 = self.liquidity_receipt_counter as u32;
            let buy_list_key = BuyListKey::new(discount_basis_points, auto_unstake, self.avl_position_counter, receipt_id_u32);
            self.avl_position_counter += 1;
            
            let id = NonFungibleLocalId::Integer(IntegerNonFungibleLocalId::new(self.liquidity_receipt_counter));

            // Mint NFT with immutable + automation data
            let liquidity_receipt_data = LiquidityReceipt {
                key_image_url: self.receipt_image_url.clone(),
                discount,
                auto_unstake,
                auto_refill,
                refill_threshold,
                automation_fee,
            };

            let new_liquidity_receipt: NonFungibleBucket = self.liquidity_receipt.mint_non_fungible(&id, liquidity_receipt_data);
            
            // Store mutable data in KVS - still track epoch for informational purposes
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), id);
            let liquidity_data = LiquidityData {
                xrd_liquidity_filled: dec!(0),
                xrd_liquidity_available: xrd_bucket.amount(),
                fills_to_collect: 0,
                last_added_epoch: Runtime::current_epoch().number() as u32,  // Keep for info only
            };
            self.liquidity_data.insert(global_id.clone(), liquidity_data);
            
            // Add to automated tracking if auto_refill is enabled
            if auto_refill {
                self.automated_liquidity.insert(self.automated_liquidity_index, global_id.clone());
                self.automated_liquidity_index += 1;
            }
            
            self.liquidity_receipt_counter += 1;

            // Use the new buy list key
            self.buy_list.insert(buy_list_key, global_id.clone());

            let index_usize = match (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>() {
                Ok(index) => index,
                Err(_) => panic!("Failed to calculate liquidity index for discount: {}", discount),
            };

            let currently_liquidity_at_discount = self.liquidity_index[index_usize];
            self.liquidity_index[index_usize] = currently_liquidity_at_discount + xrd_bucket.amount();

            self.total_xrd_locked += xrd_bucket.amount();
            
            Runtime::emit_event(LiquidityAddedEvent {
                receipt_id: global_id.local_id().clone(),
                xrd_amount: xrd_bucket.amount(),
                discount,
                auto_unstake,
                auto_refill,
                refill_threshold,
            });

            self.xrd_liquidity.put(xrd_bucket);

            new_liquidity_receipt
        }

        /// Increases existing liquidity position with additional XRD.
        /// 
        /// This method allows users to add more XRD to an existing liquidity position identified by a liquidity
        /// receipt NFT. The additional XRD is added to the available liquidity balance and the position is moved
        /// to the back of the queue for its discount level by updating its epoch. This ensures fair ordering
        /// where newly increased positions go behind existing positions at the same discount level. The method
        /// enforces the same minimum liquidity requirement as add_liquidity.
        /// 
        /// # Arguments
        /// * `receipt_bucket`: A `Bucket` containing exactly one liquidity receipt NFT
        /// * `xrd_bucket`: A `Bucket` containing XRD to be added to the existing position
        ///
        /// # Returns
        /// * A `Bucket` containing the same liquidity receipt NFT that was passed in
        pub fn increase_liquidity(&mut self, receipt_bucket: Bucket, xrd_bucket: Bucket) -> Bucket {
            assert!(receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt");
            assert!(receipt_bucket.amount() == dec!(1), "Must provide exactly one liquidity receipt");
            assert!(xrd_bucket.resource_address() == XRD, "Bucket must contain XRD");
            
            let local_id = receipt_bucket.as_non_fungible().non_fungible_local_id();
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
            let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
            
            // Check that current + new liquidity meets minimum requirement
            assert!(
                kvs_data.xrd_liquidity_available + xrd_bucket.amount() >= self.minimum_liquidity, 
                "Total liquidity after increase would be below the minimum liquidity requirement"
            );
            
            // Store the amount for the event before consuming the bucket
            let additional_xrd_amount = xrd_bucket.amount();
            
            // Get discount for the key
            let discount_basis_points = match (nft_data.discount * dec!(10000)).checked_floor() {
                Some(val) => match val.to_string().parse::<u32>() {
                    Ok(points) => points as u16,  // Convert to u16
                    Err(_) => panic!("Failed to parse discount basis points")
                },
                None => panic!("Failed to convert discount to basis points")
            };

            // Find and remove from old position
            let mut key_to_remove = None;
            self.buy_list.range_mut(0..u128::MAX).for_each(|(key, tree_global_id, _)| {
                if tree_global_id == &global_id {
                    key_to_remove = Some(*key);
                    return scrypto_avltree::IterMutControl::Break;
                }
                scrypto_avltree::IterMutControl::Continue
            });

            if let Some(key) = key_to_remove {
                self.buy_list.remove(&key);
            }

            // Update KVS data
            kvs_data.xrd_liquidity_available += additional_xrd_amount;
            let current_epoch = Runtime::current_epoch().number() as u32;
            kvs_data.last_added_epoch = current_epoch;

            // Create new key with updated position counter
            let receipt_id_u32 = match local_id.clone() {
                NonFungibleLocalId::Integer(i) => i.value() as u32,
                _ => panic!("Invalid NFT ID type")
            };
            let new_buy_list_key = BuyListKey::new(discount_basis_points, nft_data.auto_unstake, self.avl_position_counter, receipt_id_u32);
            self.avl_position_counter += 1;

            // Reinsert at new position
            self.buy_list.insert(new_buy_list_key, global_id.clone());
            
            // Update liquidity index
            let index_usize = (nft_data.discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
            self.liquidity_index[index_usize] += additional_xrd_amount;
            
            self.total_xrd_locked += additional_xrd_amount;
            self.xrd_liquidity.put(xrd_bucket);
            
            // Emit the event
            Runtime::emit_event(LiquidityIncreasedEvent {
                receipt_id: local_id,
                additional_xrd: additional_xrd_amount,
            });
            
            receipt_bucket
        }

        pub fn update_auto_refill_status(&mut self, receipt_bucket: Bucket, auto_refill: bool) -> Bucket {
            assert!(receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt");
            assert!(receipt_bucket.amount() == dec!(1), "Must provide exactly one liquidity receipt");
            
            let local_id = receipt_bucket.as_non_fungible().non_fungible_local_id();
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
            
            if auto_refill {
                assert!(nft_data.auto_unstake, "Cannot enable auto refill on a receipt that has auto unstake disabled");
                assert!(nft_data.refill_threshold >= self.minimum_refill_threshold, "Current refill threshold is below required minimum");
            }
            
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
            
            // Handle automation tracking changes
            if auto_refill && !nft_data.auto_refill {
                // Enabling automation
                self.automated_liquidity.insert(self.automated_liquidity_index, global_id);
                self.automated_liquidity_index += 1;
            } else if !auto_refill && nft_data.auto_refill {
                // Disabling automation
                let mut found_at_index = None;
                
                // Find the index to remove
                for i in 1..self.automated_liquidity_index {
                    if let Some(stored_global_id) = self.automated_liquidity.get(&i) {
                        if *stored_global_id == global_id {
                            found_at_index = Some(i);
                            break;
                        }
                    }
                }
                
                if let Some(index_to_remove) = found_at_index {
                    let last_index = self.automated_liquidity_index - 1;
                    
                    if index_to_remove == last_index {
                        // Removing the last entry - simple case
                        self.automated_liquidity.remove(&index_to_remove);
                    } else {
                        // Not the last entry - need to move last to fill gap
                        // Get the last entry first
                        let last_entry = self.automated_liquidity.get(&last_index)
                            .expect("Last entry should exist")
                            .clone();
                        
                        // Now perform both operations
                        self.automated_liquidity.remove(&index_to_remove);
                        self.automated_liquidity.remove(&last_index);
                        self.automated_liquidity.insert(index_to_remove, last_entry);
                    }
                    
                    self.automated_liquidity_index -= 1;
                }
            }
            
            // Update NFT data
            self.liquidity_receipt.update_non_fungible_data(&local_id, "auto_refill", auto_refill);
            
            // Emit the automation update event
            Runtime::emit_event(AutoRefillStatusUpdatedEvent {
                receipt_id: local_id,
                auto_refill,
            });
            
            receipt_bucket
        }

        /// Updates only the refill threshold for a liquidity receipt
        pub fn update_refill_threshold(&mut self, receipt_bucket: Bucket, refill_threshold: Decimal) -> Bucket {
            assert!(receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt");
            assert!(receipt_bucket.amount() == dec!(1), "Must provide exactly one liquidity receipt");
            assert!(refill_threshold >= self.minimum_refill_threshold, "Refill threshold is lower than the required minimum");
            
            let local_id = receipt_bucket.as_non_fungible().non_fungible_local_id();
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
            let xrd_liquidity_available = self.liquidity_data.get(&global_id).unwrap().xrd_liquidity_available;
            
            // Skip if no change
            if nft_data.refill_threshold == refill_threshold {
                return receipt_bucket;
            }
            
            // Update NFT data
            self.liquidity_receipt.update_non_fungible_data(&local_id, "refill_threshold", refill_threshold);
            
            // Emit the automation update event
            Runtime::emit_event(RefillThresholdUpdatedEvent {
                receipt_id: local_id,
                refill_threshold,
            });
            
            receipt_bucket
        }

        pub fn update_automation_fee(&mut self, receipt_bucket: Bucket, automation_fee: Decimal) -> Bucket {
            assert!(receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt");
            assert!(receipt_bucket.amount() == dec!(1), "Must provide exactly one liquidity receipt");
            assert!(automation_fee >= dec!(0), "Automation fee cannot be negative");
            
            let local_id = receipt_bucket.as_non_fungible().non_fungible_local_id();
            
            // Update NFT data
            self.liquidity_receipt.update_non_fungible_data(&local_id, "automation_fee", automation_fee);
            
            // Emit event if you want
            Runtime::emit_event(AutomationFeeUpdatedEvent {
                receipt_id: local_id,
                automation_fee,
            });
            
            receipt_bucket
        }

        /// Cycles liquidity for one or more receipts by claiming fills and re-adding as liquidity.
        /// 
        /// This method processes liquidity receipts sequentially, cycling each one only if ALL its
        /// fills can be processed within the remaining fill limit. Processing stops when the next receipt
        /// would exceed the max_fills_to_process limit. Each receipt must have both auto_unstake and 
        /// auto_refill enabled, and must meet its refill threshold. An automation fee is deducted for
        /// each receipt and accumulated into a single bucket returned to the caller. The remaining XRD 
        /// is added back to each position's available liquidity.
        /// 
        /// # Arguments
        /// * `receipt_ids`: A `Vec<NonFungibleLocalId>` containing one or more receipt IDs to cycle
        /// * `max_fills_to_process`: Maximum total fills to process across all receipts
        ///
        /// # Returns
        /// * A `FungibleBucket` containing the accumulated automation fees in XRD
        pub fn cycle_liquidity(&mut self, receipt_ids: Vec<NonFungibleLocalId>) -> FungibleBucket {
            assert!(!receipt_ids.is_empty(), "Must provide at least one receipt ID");
            
            let mut total_automation_fees = FungibleBucket::new(XRD);
            let mut fills_processed_total = 0u64;
            
            for (index, receipt_id) in receipt_ids.into_iter().enumerate() {
                // Check if we've hit our total limit
                if fills_processed_total >= self.max_fills_per_cycle {
                    break;
                }
                
                let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&receipt_id);
                let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), receipt_id.clone());
                
                // Skip if automation not enabled
                if !nft_data.auto_refill || !nft_data.auto_unstake {
                    continue;
                }
                
                // Get claimable XRD and ordered fill keys
                let (total_claimable, total_fills, ordered_keys) = self.calculate_claimable_xrd_and_ordered_list(receipt_id.clone());
                
                // Skip if no fills
                if total_fills == 0 {
                    continue;
                }
                
                // Skip if doesn't meet threshold
                if total_claimable < nft_data.refill_threshold {
                    continue;
                }
                
                let fills_available = ordered_keys.len() as u64;
                let remaining_budget = self.max_fills_per_cycle - fills_processed_total;
                
                // If this is NOT the first receipt we're processing (index > 0 or fills_processed_total > 0)
                // AND we can't complete all fills, skip this receipt
                if fills_processed_total > 0 && fills_available > remaining_budget {
                    continue;  // Don't start a receipt we can't finish
                }
                
                // Process either all fills or up to our budget
                let fills_to_process = if fills_available < remaining_budget {
                    fills_available
                } else {
                    remaining_budget
                };
                
                // Process the fills
                let mut total_xrd = FungibleBucket::new(XRD);
                let mut fills_collected = 0u64;
                
                for i in 0..fills_to_process as usize {
                    let avl_key = ordered_keys[i];
                    
                    // Get the fill from tree
                    let fill = self.order_fill_tree.get(&avl_key)
                        .expect("Fill should exist")
                        .clone();
                    
                    match fill {
                        UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                            let local_id: NonFungibleLocalId = unstake_nft_data.id.clone();
                            let nft_resource_address = unstake_nft_data.resource_address;
                            
                            // Get the unstake NFT from vault
                            let unstake_nft_vault = self.component_vaults.get_mut(&nft_resource_address).unwrap();
                            let unstake_nft = unstake_nft_vault.as_non_fungible().take_non_fungible(&local_id);
                            
                            // Get validator address - avoid borrowing self immutably while mutably borrowed
                            let metadata: GlobalAddress = ResourceManager::from(nft_resource_address)
                                .get_metadata("validator")
                                .unwrap()
                                .unwrap_or_else(|| Runtime::panic(String::from("Not an unstake NFT!")));
                            let validator_address = ComponentAddress::try_from(metadata).unwrap();
                            let mut validator: Global<Validator> = Global::from(validator_address);
                            
                            let claimed_xrd = validator.claim_xrd(unstake_nft);
                            
                            total_xrd.put(claimed_xrd);
                        },
                        UnstakeNFTOrLSU::LSU(_) => {
                            panic!("Cannot cycle LSU fills - receipt must have auto_unstake enabled");
                        }
                    }
                    
                    // Remove the processed fill
                    self.order_fill_tree.remove(&avl_key);
                    fills_collected += 1;
                }
                
                // Update KVS data
                let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
                kvs_data.fills_to_collect = kvs_data.fills_to_collect.saturating_sub(fills_collected);
                
                // Skip if we didn't collect enough to cover fees
                if total_xrd.amount() < self.automation_fee {
                    // This shouldn't happen with proper ordering, but just in case
                    self.xrd_liquidity.put(total_xrd.into());
                    continue;
                }
                let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&receipt_id);
        
                
                // Take automation fee
                let fee_amount = nft_data.automation_fee;
                let automation_fee_bucket = total_xrd.take(fee_amount);
                total_automation_fees.put(automation_fee_bucket);
                
                // Find and remove from current position in AVL tree
                let mut key_to_remove = None;
                for (key, tree_global_id, _) in self.buy_list.range(0..u128::MAX) {
                    if tree_global_id == global_id {
                        key_to_remove = Some(key);
                        break;
                    }
                }
                
                if let Some(key) = key_to_remove {
                    self.buy_list.remove(&key);
                }
                
                // Add remaining XRD back to liquidity
                let xrd_to_add = total_xrd.amount();
                
                // Update KVS data
                kvs_data.xrd_liquidity_available += xrd_to_add;
                let current_epoch = Runtime::current_epoch().number() as u32;
                kvs_data.last_added_epoch = current_epoch;
                
                // Create new key with new position
                let discount_basis_points = match (nft_data.discount * dec!(10000)).checked_floor() {
                    Some(val) => match val.to_string().parse::<u32>() {
                        Ok(points) => points as u16,
                        Err(_) => panic!("Failed to parse discount basis points")
                    },
                    None => panic!("Failed to convert discount to basis points")
                };
                
                let receipt_id_u32 = match receipt_id.clone() {
                    NonFungibleLocalId::Integer(i) => i.value() as u32,
                    _ => panic!("Invalid NFT ID type")
                };
                
                let new_buy_list_key = BuyListKey::new(discount_basis_points, nft_data.auto_unstake, self.avl_position_counter, receipt_id_u32);
                self.avl_position_counter += 1;
                
                // Reinsert at new position
                self.buy_list.insert(new_buy_list_key, global_id);
                
                // Update liquidity index
                let index_usize = (nft_data.discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
                self.liquidity_index[index_usize] += xrd_to_add;
                
                // Put XRD in vault
                self.xrd_liquidity.as_fungible().put(total_xrd);
                self.total_xrd_locked += xrd_to_add;
                
                // Emit the cycle event
                Runtime::emit_event(LiquidityCycledEvent {
                    receipt_id: receipt_id.clone(),
                    xrd_amount_cycled: xrd_to_add,
                    automation_fee: fee_amount,
                });
                
                fills_processed_total += fills_collected;
            }
            
            // Return the automation fees to the caller - this must be the last statement
            total_automation_fees
        }

        pub fn calculate_claimable_xrd_and_ordered_list(&self, receipt_id: NonFungibleLocalId) -> (Decimal, u64, Vec<u128>) {
            // Returns: (total_claimable_now, total_fills, vec_of_avl_keys_ordered_by_amount_desc)
            
            let receipt_id_u64 = match receipt_id {
                NonFungibleLocalId::Integer(i) => i.value(),
                _ => return (dec!(0), 0, vec![]),
            };
            
            let start_key = OrderFillKey::new(receipt_id_u64, 1);
            let end_key = OrderFillKey::new(receipt_id_u64, u64::MAX);
            
            let current_epoch = Runtime::current_epoch().number() as u64;
            let mut claimable_fills: Vec<(u128, Decimal)> = Vec::new();
            let mut total_claimable_now = dec!(0);
            let mut total_fills = 0u64;
            
            for (key, value, _) in self.order_fill_tree.range(start_key..=end_key) {
                total_fills += 1;
                
                // We know it's an UnstakeNFT because this is only called for auto_refill receipts
                match value {
                    UnstakeNFTOrLSU::UnstakeNFT(unstake_data) => {
                        let unstake_nft_resource = ResourceManager::from_address(unstake_data.resource_address);
                        let nft_data: UnstakeData = unstake_nft_resource.get_non_fungible_data(&unstake_data.id);
                        
                        // Only include if claimable now
                        if current_epoch >= nft_data.claim_epoch.number() {
                            total_claimable_now += nft_data.claim_amount;
                            claimable_fills.push((key, nft_data.claim_amount));
                        }
                    },
                    UnstakeNFTOrLSU::LSU(_) => {
                        panic!("Found LSU in auto_refill receipt - this should never happen!");
                    }
                }
            }
            
            // Sort by amount descending (largest first), then extract just the keys
            claimable_fills.sort_by(|a, b| b.1.cmp(&a.1));
            let ordered_keys: Vec<u128> = claimable_fills.into_iter().map(|(key, _)| key).collect();
            
            (total_claimable_now, total_fills, ordered_keys)
        }



        /// Removes liquidity and returns XRD to the provider.
        /// 
        /// This method allows liquidity providers to withdraw their available XRD liquidity. Only XRD that
        /// hasn't been used to fill orders can be withdrawn - any fills must be collected separately.
        /// If the position has auto_refill enabled, it will be disabled and removed from automation tracking.
        /// The position is removed from the buy list order book and the liquidity index is updated. Multiple
        /// receipts can be processed in a single transaction.
        /// 
        /// # Arguments
        /// * `liquidity_receipt_bucket`: A `Bucket` containing one or more liquidity receipt NFTs
        ///
        /// # Returns
        /// * A tuple containing:
        ///   - `Bucket`: The withdrawn XRD from all provided receipts
        ///   - `Bucket`: The liquidity receipt NFTs (returned unchanged)
        pub fn remove_liquidity(&mut self, liquidity_receipt_bucket: Bucket) -> (Bucket, Bucket) {
            assert!(liquidity_receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt(s)");

            // First pass: Collect all data and validate
            let mut removal_data: Vec<(NonFungibleLocalId, NonFungibleGlobalId, LiquidityReceipt, Decimal, Decimal, usize)> = Vec::new();
            let mut total_order_size = Decimal::ZERO;
            
            for local_id in liquidity_receipt_bucket.as_non_fungible().non_fungible_local_ids() {
                let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
                let kvs_data = self.liquidity_data.get(&global_id).unwrap();
                assert!(kvs_data.xrd_liquidity_available > dec!(0), "No liquidity available to remove");
                
                let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
                let order_size = kvs_data.xrd_liquidity_available;
                let discount = nft_data.discount;
                let index = (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
                
                removal_data.push((local_id.clone(), global_id, nft_data, order_size, discount, index));
                total_order_size += order_size;
            }

            // Second pass: Update NFT data for auto_refill receipts
            let mut automated_removals: Vec<NonFungibleGlobalId> = Vec::new();
            for (local_id, global_id, nft_data, _, _, _) in &removal_data {
                if nft_data.auto_refill {
                    self.liquidity_receipt.update_non_fungible_data(local_id, "auto_refill", false);
                    automated_removals.push(global_id.clone());
                }
            }

            // Third pass: Remove from automated liquidity tracking
            for global_id_to_remove in automated_removals {
                let mut target_index = None;
                let mut last_entry_data = None;
                
                // Find the target and get last entry data if needed
                for i in 1..self.automated_liquidity_index {
                    if let Some(stored_global_id) = self.automated_liquidity.get(&i) {
                        if *stored_global_id == global_id_to_remove {
                            target_index = Some(i);
                        }
                        // Store the last entry data
                        if i == self.automated_liquidity_index - 1 {
                            last_entry_data = Some(stored_global_id.clone());
                        }
                    }
                }
                
                // Now do the removal and reshuffling
                if let Some(index_to_remove) = target_index {
                    let last_index = self.automated_liquidity_index - 1;
                    
                    if index_to_remove == last_index {
                        // Removing the last entry - simple case
                        self.automated_liquidity.remove(&index_to_remove);
                    } else {
                        // Not the last entry - need to move last to fill gap
                        // Remove the target entry
                        self.automated_liquidity.remove(&index_to_remove);
                        
                        if let Some(last_entry) = last_entry_data {
                            // Remove from last position
                            self.automated_liquidity.remove(&last_index);
                            // Insert at the gap position
                            self.automated_liquidity.insert(index_to_remove, last_entry);
                        }
                    }
                    
                    self.automated_liquidity_index -= 1;
                }
            }

            // Fourth pass: Update liquidity index
            for (_, _, _, order_size, _, index) in &removal_data {
                self.liquidity_index[*index] -= *order_size;
            }

            // Fifth pass: Find all keys to remove from buy list
            let mut keys_to_remove: Vec<u128> = Vec::new();
            for (_, global_id, _, _, _, _) in &removal_data {
                for (key, tree_global_id, _) in self.buy_list.range(0..u128::MAX) {
                    if tree_global_id == global_id.clone() {
                        keys_to_remove.push(key);
                        break;
                    }
                }
            }

            // Remove from buy list
            for key in keys_to_remove {
                self.buy_list.remove(&key);
            }

            // Sixth pass: Update KVS data
            for (_, global_id, _, _, _, _) in &removal_data {
                let mut kvs_data = self.liquidity_data.get_mut(global_id).unwrap();
                kvs_data.xrd_liquidity_available = dec!(0);
            }

            // Take XRD from vault
            let xrd_bucket = self.xrd_liquidity.take(total_order_size);
            self.total_xrd_locked -= total_order_size;

            // Emit events
            for (local_id, _, _, order_size, _, _) in removal_data {
                Runtime::emit_event(LiquidityRemovedEvent {
                    receipt_id: local_id,
                    xrd_amount: order_size,
                });
            }

            (xrd_bucket, liquidity_receipt_bucket)
        }

        /// Processes LSU unstaking using on-ledger order matching.
        /// 
        /// This method takes LSUs and matches them against available liquidity positions in discount order.
        /// The matching algorithm iterates through the buy list up to max_iterations times, filling orders
        /// at progressively worse discounts until all LSUs are processed or no more liquidity is available.
        /// Filled orders result in either LSUs (if auto_unstake is false) or unstake NFTs (if auto_unstake
        /// is true) being stored for later collection by liquidity providers.
        /// 
        /// # Arguments
        /// * `lsu_bucket`: A `FungibleBucket` containing native Radix validator LSUs
        /// * `max_iterations`: A `u8` limiting the number of liquidity positions to check
        ///
        /// # Returns
        /// * A tuple containing:
        ///   - `Bucket`: XRD received from the liquidity providers (minus platform fee)
        ///   - `FungibleBucket`: Any remaining LSUs that couldn't be matched
        pub fn liquify_unstake(&mut self, lsu_bucket: FungibleBucket, max_iterations: u8) -> (Bucket, FungibleBucket) {
            assert!(self.validate_lsu(lsu_bucket.resource_address()), "Bucket must contain a native Radix Validator LSU");

            // Pre-calculate total value and validate min/max
            let validator = self.get_validator_from_lsu(lsu_bucket.resource_address());
            let redemption_rate = validator.get_redemption_value(dec!(1));
            let total_lsu_value = lsu_bucket.amount() * redemption_rate;
            
            // Enforce min/max unstake values
            assert!(
                total_lsu_value >= self.minimum_unstake_value,
                "Unstake value of {} XRD is below minimum requirement of {} XRD",
                total_lsu_value,
                self.minimum_unstake_value
            );
            assert!(
                total_lsu_value <= self.maximum_unstake_value,
                "Unstake value of {} XRD exceeds maximum limit of {} XRD",
                total_lsu_value,
                self.maximum_unstake_value
            );
            
            // Check if this is a small order
            let is_small_order = total_lsu_value < self.small_order_threshold;

            let mut order_keys = Vec::new();
            let mut iter_count = 0;
            
            self.buy_list.range_mut(0..u128::MAX).for_each(|(avl_key, _global_id, _)| {
                if iter_count >= max_iterations as usize {
                    return scrypto_avltree::IterMutControl::Break;
                }
                
                // For small orders, only include keys with auto_unstake=true
                if is_small_order && !BuyListKey::extract_auto_unstake(*avl_key) {
                    return scrypto_avltree::IterMutControl::Continue;
                }
                
                order_keys.push(*avl_key);
                iter_count += 1;
                
                scrypto_avltree::IterMutControl::Continue
            });
            
            self.process_unstake(lsu_bucket, order_keys)
        }

        /// Processes LSU unstaking using off-ledger computed order keys.
        /// 
        /// This method enables more efficient order matching by allowing order keys to be computed off-ledger
        /// and passed in directly. This avoids the iteration limits of the on-ledger method and enables
        /// sophisticated matching algorithms to run off-chain. The order keys must correspond to valid
        /// positions in the buy list AVL tree. Invalid keys are skipped without causing transaction failure.
        /// 
        /// # Arguments
        /// * `lsu_bucket`: A `FungibleBucket` containing native Radix validator LSUs
        /// * `order_keys`: A `Vec<u128>` of pre-computed AVL tree keys to match against
        ///
        /// # Returns
        /// * A tuple containing:
        ///   - `Bucket`: XRD received from the liquidity providers (minus platform fee)
        ///   - `FungibleBucket`: Any remaining LSUs that couldn't be matched
        pub fn liquify_unstake_off_ledger(&mut self, lsu_bucket: FungibleBucket, order_keys: Vec<u128>) -> (Bucket, FungibleBucket) {
            assert!(self.validate_lsu(lsu_bucket.resource_address()), "Bucket must contain a native Radix Validator LSU");
            
            // Pre-calculate total value and validate min/max
            let validator = self.get_validator_from_lsu(lsu_bucket.resource_address());
            let redemption_rate = validator.get_redemption_value(dec!(1));
            let total_lsu_value = lsu_bucket.amount() * redemption_rate;
            
            // Enforce min/max unstake values
            assert!(
                total_lsu_value >= self.minimum_unstake_value,
                "Unstake value of {} XRD is below minimum requirement of {} XRD",
                total_lsu_value,
                self.minimum_unstake_value
            );
            assert!(
                total_lsu_value <= self.maximum_unstake_value,
                "Unstake value of {} XRD exceeds maximum limit of {} XRD",
                total_lsu_value,
                self.maximum_unstake_value
            );
            
            // Check if this is a small order
            let is_small_order = total_lsu_value < self.small_order_threshold;
            
            // Filter order keys if it's a small order
            let filtered_keys = if is_small_order {
                order_keys.into_iter()
                    .filter(|key| !BuyListKey::extract_auto_unstake(*key))
                    .collect()
            } else {
                order_keys
            };
            
            self.process_unstake(lsu_bucket, filtered_keys)
        }

        fn process_unstake(&mut self, mut lsu_bucket: FungibleBucket, order_keys: Vec<u128>) -> (Bucket, FungibleBucket) {
            
            let mut xrd_bucket: Bucket = Bucket::new(XRD);
            let mut validator = self.get_validator_from_lsu(lsu_bucket.resource_address());
            
            // Store initial values for event
            let lsu_resource = lsu_bucket.resource_address();
            let initial_lsu_amount = lsu_bucket.amount();
            
            // Calculate redemption rate
            let redemption_rate = validator.get_redemption_value(dec!(1));
            
            let mut remaining_lsus = lsu_bucket.amount();
            let mut remaining_value = remaining_lsus * redemption_rate;

            // Batch collections
            let mut avl_removals = Vec::new();
            let mut kvs_updates: Vec<(NonFungibleGlobalId, Decimal, Decimal, u64)> = Vec::new();
            let mut index_updates: std::collections::HashMap<usize, Decimal> = std::collections::HashMap::new();
            
            // Separate fill operations by type to batch vault operations
            let mut unstake_operations: Vec<(u128, FungibleBucket)> = Vec::new();
            let mut lsu_operations: Vec<(u128, ResourceAddress, Decimal)> = Vec::new();
            let mut vault_resources_needed: std::collections::HashSet<ResourceAddress> = std::collections::HashSet::new();

            for key in order_keys {
                let global_id_option = self.buy_list.get(&key);
                if global_id_option.is_none() {
                    continue;
                }
                
                let global_id = global_id_option.unwrap().clone();
                let local_id = global_id.local_id();
                
                // Read data once
                let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
                let discount = nft_data.discount;
                let auto_unstake = nft_data.auto_unstake;
                
                let kvs_data = self.liquidity_data.get(&global_id).unwrap();
                let xrd_available = kvs_data.xrd_liquidity_available;
                let current_fills = kvs_data.fills_to_collect;

                // Calculate fill
                let discounted_value = remaining_value * (dec!(1) - discount);
                let (lsu_to_take, fill_amount, new_xrd_available) = if discounted_value <= xrd_available {
                    (remaining_lsus, discounted_value, xrd_available - discounted_value)
                } else {
                    let lsu_ratio = xrd_available / discounted_value;
                    let lsu_take = remaining_lsus * lsu_ratio;
                    (lsu_take, xrd_available, dec!(0))
                };

                // Take resources
                let lsu_taken: FungibleBucket = lsu_bucket.take(lsu_to_take);
                let xrd_funds = self.xrd_liquidity.take(fill_amount);
                xrd_bucket.put(xrd_funds);

                // Update tracking
                remaining_lsus -= lsu_to_take;
                remaining_value = remaining_lsus * redemption_rate;

                let local_id_u64 = match local_id {
                    NonFungibleLocalId::Integer(i) => i.value(),
                    _ => 0,
                };

                // Queue updates
                if new_xrd_available == dec!(0) {
                    avl_removals.push(key);
                }
                
                kvs_updates.push((global_id.clone(), new_xrd_available, fill_amount, current_fills + 1));
                
                // Aggregate index updates
                let index = (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
                *index_updates.entry(index).or_insert(dec!(0)) += fill_amount;

                // Create order fill key using new structure
                let order_fill_key = OrderFillKey::new(local_id_u64, self.order_fill_counter);
                self.order_fill_counter += 1;
                
                if auto_unstake {
                    unstake_operations.push((order_fill_key, lsu_taken));
                } else {
                    let resource = lsu_taken.resource_address();
                    vault_resources_needed.insert(resource);
                    lsu_operations.push((order_fill_key, resource, lsu_taken.amount()));
                    // Put LSU in temporary storage
                    if !self.component_vaults.get(&resource).is_some() {
                        self.component_vaults.insert(resource, Vault::new(resource));
                    }
                    self.component_vaults.get_mut(&resource).unwrap().as_fungible().put(lsu_taken);
                }
                
                // Emit OrderFillEvent for this fill
                Runtime::emit_event(OrderFillEvent {
                    receipt_id: local_id.clone(),
                    lsu_amount: lsu_to_take,
                    xrd_amount: fill_amount,
                    discount,
                });

                if remaining_lsus.is_zero() {
                    break;
                }
            }

            // Batch apply all non-vault updates first
            for key in avl_removals {
                self.buy_list.remove(&key);
            }

            for (global_id, new_available, fill_amount, new_fills) in kvs_updates {
                let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
                kvs_data.xrd_liquidity_filled += fill_amount;
                kvs_data.xrd_liquidity_available = new_available;
                kvs_data.fills_to_collect = new_fills;
            }

            for (index, total_fill) in index_updates {
                self.liquidity_index[index] -= total_fill;
            }

            // Process LSU fills (already in vaults)
            for (order_fill_key, resource, amount) in lsu_operations {
                let lsu_data = UnstakeNFTOrLSU::LSU(LSUData { resource_address: resource, amount });
                self.order_fill_tree.insert(order_fill_key, lsu_data);
            }

            // Batch process all unstake operations
            if !unstake_operations.is_empty() {
                // Get the first unstake to determine NFT resource
                let (first_key, first_lsu) = unstake_operations.remove(0);
                let first_nft = validator.unstake(first_lsu);
                let nft_resource = first_nft.resource_address();
                
                // Ensure vault exists once
                if !self.component_vaults.get(&nft_resource).is_some() {
                    self.component_vaults.insert(nft_resource, Vault::new(nft_resource));
                }
                
                // Process first NFT
                let nft_vault = self.component_vaults.get_mut(&nft_resource).unwrap();
                let first_id = first_nft.non_fungible_local_id();
                self.order_fill_tree.insert(first_key, UnstakeNFTOrLSU::UnstakeNFT(UnstakeNFTData {
                    resource_address: nft_resource,
                    id: first_id,
                }));
                nft_vault.as_non_fungible().put(first_nft);
                
                // Process remaining unstakes
                for (order_fill_key, lsu_bucket) in unstake_operations {
                    let unstake_nft = validator.unstake(lsu_bucket);
                    let nft_id = unstake_nft.non_fungible_local_id();
                    self.order_fill_tree.insert(order_fill_key, UnstakeNFTOrLSU::UnstakeNFT(UnstakeNFTData {
                        resource_address: nft_resource,
                        id: nft_id,
                    }));
                    nft_vault.as_non_fungible().put(unstake_nft);
                }
            }

            // Update totals and fees
            self.total_xrd_volume += xrd_bucket.amount();
            self.total_xrd_locked -= xrd_bucket.amount();

            let fee_bucket = xrd_bucket.take(xrd_bucket.amount() * self.platform_fee);
            self.fee_vault.put(fee_bucket);
            
            // Calculate actual amounts for event
            let lsu_amount_processed = initial_lsu_amount - lsu_bucket.amount();
            let xrd_received = xrd_bucket.amount();
            
            // Emit the unstake event
            Runtime::emit_event(LiquifyUnstakeEvent {
                lsu_resource,
                lsu_amount: lsu_amount_processed,
                xrd_received,
            });
            
            (xrd_bucket, lsu_bucket)
        }
        
        /// Collects fills for liquidity providers.
        /// 
        /// This method allows liquidity providers to collect LSUs or unstake NFTs from orders they've filled.
        /// The number of fills to collect can be limited to manage transaction costs. Fills are returned in
        /// the order they were created. For positions with auto_unstake enabled, unstake NFTs are returned.
        /// For positions without auto_unstake, the original LSUs are returned. Multiple receipts can be
        /// processed in one transaction.
        /// 
        /// # Arguments
        /// * `liquidity_receipt_bucket`: A `Bucket` containing one or more liquidity receipt NFTs
        /// * `number_of_fills_to_collect`: A `u64` limiting total fills collected across all receipts
        ///
        /// # Returns
        /// * A tuple containing:
        ///   - `Vec<Bucket>`: A vector of buckets containing collected LSUs or unstake NFTs
        ///   - `Bucket`: The liquidity receipt NFTs (returned unchanged)
        pub fn collect_fills(&mut self, liquidity_receipt_bucket: Bucket, number_of_fills_to_collect: u64) -> (Vec<Bucket>, Bucket) {
            
            assert!(
                liquidity_receipt_bucket.resource_address() == self.liquidity_receipt.address(),
                "Bucket must contain Liquify liquidity receipts NFT(s)"
            );

            let mut bucket_vec: Vec<Bucket> = Vec::new();
            let mut collect_counter: u64 = 0;
            let mut all_updates = vec![];
            
            // Track event data
            let mut event_data_per_receipt: HashMap<NonFungibleLocalId, (u64, Vec<(Decimal, ResourceAddress)>, Vec<NonFungibleGlobalId>)> = HashMap::new();

            for order_id in liquidity_receipt_bucket.as_non_fungible().non_fungible_local_ids() {
                let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), order_id.clone());
                let fills_to_collect = {
                    let kvs_data = self.liquidity_data.get(&global_id).unwrap();
                    kvs_data.fills_to_collect
                };

                if fills_to_collect == 0 {
                    continue;
                }

                if collect_counter >= number_of_fills_to_collect {
                    break;
                }

                let order_id_u64 = match order_id.clone() {
                    NonFungibleLocalId::Integer(i) => i.value(),
                    _ => 0,
                };

                // Use OrderFillKey for searching
                let start_key = OrderFillKey::new(order_id_u64, 1);
                let end_key = OrderFillKey::new(order_id_u64, u64::MAX);

                let mut fills_collected_for_this_order: u64 = 0;
                let mut fills_to_remove = Vec::new();
                let mut lsus_collected = Vec::new();
                let mut stake_claim_nfts_collected = Vec::new();

                // First, collect the fills we need to process
                for (key, value, _) in self.order_fill_tree.range(start_key..=end_key) {
                    if collect_counter >= number_of_fills_to_collect {
                        break;
                    }

                    fills_to_remove.push((key, value.clone()));
                    fills_collected_for_this_order += 1;
                    collect_counter += 1;
                }

                // Process the collected fills
                for (avl_key, unstake_nft_or_lsu) in fills_to_remove {
                    match unstake_nft_or_lsu {
                        UnstakeNFTOrLSU::LSU(lsu_data) => {
                            let mut lsu_bucket = Bucket::new(lsu_data.resource_address);
                            let lsu_resource = lsu_data.resource_address;
                            let lsu_amount = lsu_data.amount;
                            let mut lsu_vault = self.component_vaults.get_mut(&lsu_resource).unwrap();
                            lsu_bucket.put(lsu_vault.take(lsu_amount));
                            
                            // Track for event
                            lsus_collected.push((lsu_amount, lsu_resource));
                            
                            bucket_vec.push(lsu_bucket);
                        }

                        UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                            let mut unstake_nft_bucket = Bucket::new(unstake_nft_data.resource_address);
                            let unstake_nft_id = &unstake_nft_data.id;
                            let unstake_nft_vault = self.component_vaults.get_mut(&unstake_nft_data.resource_address).unwrap();
                            unstake_nft_bucket.put(unstake_nft_vault.as_non_fungible().take_non_fungible(&unstake_nft_id).into());
                            
                            // Track for event
                            let nft_global_id = NonFungibleGlobalId::new(unstake_nft_data.resource_address, unstake_nft_id.clone());
                            stake_claim_nfts_collected.push(nft_global_id);
                            
                            bucket_vec.push(unstake_nft_bucket);
                        }
                    }

                    self.order_fill_tree.remove(&avl_key);
                }

                let new_fills_to_collect = fills_to_collect - fills_collected_for_this_order;
                all_updates.push((global_id, new_fills_to_collect));
                
                // Store event data for this receipt
                if fills_collected_for_this_order > 0 {
                    event_data_per_receipt.insert(
                        order_id.clone(),
                        (fills_collected_for_this_order, lsus_collected, stake_claim_nfts_collected)
                    );
                }
            }

            // Apply all KVS updates
            for (global_id, new_fills_to_collect) in all_updates {
                let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
                kvs_data.fills_to_collect = new_fills_to_collect;
            }
            
            // Emit events for each receipt that had fills collected
            for (receipt_id, (fills_collected, lsus, nfts)) in event_data_per_receipt {
                Runtime::emit_event(CollectFillsEvent {
                    receipt_id,
                    fills_collected,
                    lsus_collected: lsus,
                    stake_claim_nfts_collected: nfts,
                });
            }

            (bucket_vec, liquidity_receipt_bucket)
        }
                
        /// Collects accumulated platform fees.
        /// 
        /// This method allows the component owner to withdraw all platform fees that have been collected
        /// from unstaking operations. Platform fees are charged as a percentage of XRD volume processed.
        /// Only the holder of the owner badge can call this method. The fee vault is completely emptied.
        /// 
        /// # Arguments
        /// * None
        ///
        /// # Returns
        /// * A `Bucket` containing all accumulated platform fees in XRD
        pub fn collect_platform_fees(&mut self) -> Bucket {
            self.fee_vault.take_all()
        }
        
        /// Sets the operational status of the component.
        /// 
        /// This method allows the owner to enable or disable the component's ability to accept new liquidity.
        /// When disabled, add_liquidity will reject new deposits but all other operations continue to function
        /// normally. This provides a mechanism for maintenance or emergency situations without disrupting
        /// existing positions. Only the holder of the owner badge can call this method.
        /// 
        /// # Arguments
        /// * `status`: A `bool` where true enables the component and false disables it
        ///
        /// # Returns
        /// * None
        pub fn set_component_status(&mut self, status: bool) {
            self.component_status = status;
        }

        /// Sets the platform fee percentage.
        /// 
        /// This method allows the owner to adjust the platform fee charged on unstaking operations. The fee
        /// is taken from the XRD amount paid to unstakers before they receive their funds. Fee changes only
        /// affect future unstaking operations, not existing fills. Only the holder of the owner badge can
        /// call this method.
        /// 
        /// # Arguments
        /// * `fee`: A `Decimal` representing the platform fee as a percentage (e.g., 0.01 for 1%)
        ///
        /// # Returns
        /// * None
        pub fn set_platform_fee(&mut self, fee: Decimal) {
            self.platform_fee = fee;
        }

        /// Sets the minimum liquidity requirement.
        /// 
        /// This method allows the owner to adjust the minimum XRD amount required for add_liquidity and
        /// increase_liquidity operations. Higher minimums improve transaction efficiency by ensuring
        /// positions can fill meaningful order sizes, but may exclude smaller liquidity providers.
        /// Only the holder of the owner badge can call this method.
        /// 
        /// # Arguments
        /// * `min`: A `Decimal` representing the minimum XRD amount required for liquidity operations
        ///
        /// # Returns
        /// * None
        pub fn set_minimum_liquidity(&mut self, min: Decimal) {
            self.minimum_liquidity = min;
        }

        pub fn set_minimum_refill_threshold(&mut self, min: Decimal) {
            self.minimum_refill_threshold = min;
        }

        // Controls the number of iterations allowed when using cycle_liquidity to prevent hitting limits during transactions
        pub fn set_max_fills_per_cycle(&mut self, max_fills: u64) {
            self.max_fills_per_cycle = max_fills;
        }

        pub fn set_unstake_value_range(&mut self, min: Decimal, max: Decimal) {
            self.minimum_unstake_value = min;
            self.maximum_unstake_value = max;
        }

        /// Sets the small order threshold.
        /// 
        /// This method allows the owner to adjust the threshold below which orders are considered "small"
        /// and will only match with liquidity positions that have auto_unstake enabled. This helps ensure
        /// small orders don't get stuck with manual liquidity positions that may not be actively managed.
        /// Only the holder of the owner badge can call this method.
        /// 
        /// # Arguments
        /// * `threshold`: A `Decimal` representing the XRD value threshold for small orders
        ///
        /// # Returns
        /// * None
        pub fn set_small_order_threshold(&mut self, threshold: Decimal) {
            assert!(threshold >= dec!(0), "Small order threshold cannot be negative");
            self.small_order_threshold = threshold;
        }

        /// Sets the receipt NFT image URL.
        /// 
        /// This method allows the owner to update the image URL used for newly minted liquidity receipt
        /// NFTs. Existing NFTs are not affected as the URL is stored as immutable data at mint time.
        /// This allows for branding updates or fixing broken image links. Only the holder of the owner
        /// badge can call this method.
        /// 
        /// # Arguments
        /// * `url`: A `String` containing the new image URL for liquidity receipt NFTs
        ///
        /// # Returns
        /// * None
        pub fn set_receipt_image_url(&mut self, url: String) {
            self.receipt_image_url = Url::of(url);
        }

        fn get_validator_from_lsu(&self, lsu_address: ResourceAddress) -> Global<Validator> {
            let metadata: GlobalAddress = ResourceManager::from(lsu_address)
                .get_metadata("validator")
                .unwrap()
                .unwrap_or_else(|| Runtime::panic(String::from("Not an LSU!")));

            let validator_address = ComponentAddress::try_from(metadata).unwrap();
            let validator: Global<Validator> = Global::from(validator_address);

            validator
        }

        fn get_validator_from_unstake_nft(&self, nft_address: &ResourceAddress) -> ComponentAddress {
            let metadata: GlobalAddress = ResourceManager::from(*nft_address)
                .get_metadata("validator")
                .unwrap()
                .unwrap_or_else(|| Runtime::panic(String::from("Not an unstake NFT!")));

            ComponentAddress::try_from(metadata).unwrap()
        }

        fn validate_lsu(&self, input_lsu_address: ResourceAddress) -> bool {
            let validator = self.get_validator_from_lsu(input_lsu_address);

            let lsu_address: GlobalAddress = validator
                .get_metadata("pool_unit")
                .unwrap()
                .unwrap_or_else(|| Runtime::panic(String::from("Not an LSU!")));
            
            let is_valid = input_lsu_address == ResourceAddress::try_from(lsu_address).unwrap();

            is_valid
        }

        pub fn get_receipt_detail(&self, receipt_id: NonFungibleLocalId) -> ReceiptDetailData {
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&receipt_id);
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), receipt_id.clone());
            
            // Get all values from calculate_claimable_xrd
            let (claimable_xrd, total_fills, total_stake_claim_value, total_lsu_redemption_value) = 
                self.calculate_claimable_xrd(&receipt_id);
            
            let (xrd_liquidity_available, xrd_liquidity_filled, fills_to_collect, last_added_epoch) = 
                match self.liquidity_data.get(&global_id) {
                    Some(kvs_data) => (
                        kvs_data.xrd_liquidity_available,
                        kvs_data.xrd_liquidity_filled,
                        kvs_data.fills_to_collect,
                        kvs_data.last_added_epoch,
                    ),
                    None => (dec!(0), dec!(0), 0, 0)
                };

            let liquidity_ahead = self.calculate_liquidity_ahead(&receipt_id);
            
            ReceiptDetailData {
                receipt_id,
                discount: nft_data.discount,
                auto_unstake: nft_data.auto_unstake,
                auto_refill: nft_data.auto_refill,
                refill_threshold: nft_data.refill_threshold,
                xrd_liquidity_available,
                liquidity_ahead,
                xrd_liquidity_filled,
                fills_to_collect,
                last_added_epoch,
                claimable_xrd,
                total_stake_claim_value,
                total_lsu_redemption_value,
            }
        }

        pub fn get_automation_ready_receipts(&self, start_index: u64, batch_size: u64) -> Vec<AutomationReadyReceipt> {
            let mut ready_receipts = Vec::new();
            let mut checked_count = 0u64;
            
            let start = std::cmp::max(start_index, 1);
            let end = self.automated_liquidity_index;
            
            for i in start..end {
                if let Some(global_id) = self.automated_liquidity.get(&i) {
                    let receipt_id = global_id.local_id().clone();
                    let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&receipt_id);
                    
                    // Calculate claimable XRD (only need first value)
                    let (claimable_xrd, fills_to_collect, _, _) = self.calculate_claimable_xrd(&receipt_id);
                    
                    if claimable_xrd >= nft_data.refill_threshold {
                        
                        ready_receipts.push(AutomationReadyReceipt {
                            receipt_id,
                            discount: nft_data.discount,
                            fills_to_collect,
                            claimable_xrd,
                            refill_threshold: nft_data.refill_threshold,
                            automation_fee: nft_data.automation_fee,
                        });
                        
                        if ready_receipts.len() >= batch_size as usize {
                            break;
                        }
                    }
                    
                    checked_count += 1;
                    // Optional: limit how many positions we check to prevent gas issues
                    if checked_count >= batch_size * 10 {
                        break;
                    }
                }
            }
            
            ready_receipts
        }

        /// Calculates comprehensive fill information for a receipt in a single pass
        /// Returns (claimable_xrd_now, total_fill_count, total_stake_claim_value, total_lsu_redemption_value)
        fn calculate_claimable_xrd(&self, receipt_id: &NonFungibleLocalId) -> (Decimal, u64, Decimal, Decimal) {
            let receipt_id_u64 = match receipt_id {
                NonFungibleLocalId::Integer(i) => i.value(),
                _ => return (dec!(0), 0, dec!(0), dec!(0)),
            };
            
            // Use OrderFillKey for searching
            let start_key = OrderFillKey::new(receipt_id_u64, 1);
            let end_key = OrderFillKey::new(receipt_id_u64, u64::MAX);
            
            let mut claimable_now = dec!(0);
            let mut total_stake_claim_value = dec!(0);
            let mut total_lsu_redemption_value = dec!(0);
            let mut fill_count = 0u64;
            let current_epoch = Runtime::current_epoch().number() as u64;
            
            for (_, value, _) in self.order_fill_tree.range(start_key..=end_key) {
                fill_count += 1;
                
                match value {
                    UnstakeNFTOrLSU::UnstakeNFT(unstake_data) => {
                        // Get the unstake NFT data
                        let unstake_nft_resource = NonFungibleResourceManager::from(unstake_data.resource_address);
                        let nft_data: UnstakeData = unstake_nft_resource.get_non_fungible_data(&unstake_data.id);
                        
                        // Always add to total stake claim value
                        total_stake_claim_value += nft_data.claim_amount;
                        
                        // Only add to claimable now if past unbonding
                        if current_epoch >= nft_data.claim_epoch.number() {
                            claimable_now += nft_data.claim_amount;
                        }
                    },
                    UnstakeNFTOrLSU::LSU(lsu_data) => {
                        // Get the validator component from the LSU
                        let validator = self.get_validator_from_lsu(lsu_data.resource_address);
                        
                        // Calculate redemption value for this LSU
                        let redemption_value = validator.get_redemption_value(lsu_data.amount);
                        total_lsu_redemption_value += redemption_value;
                    }
                }
            }
            
            (claimable_now, fill_count, total_stake_claim_value, total_lsu_redemption_value)
        }


        /// Returns comprehensive fill information for a receipt
        /// Returns (claimable_xrd_now, total_fills, total_stake_claim_value, total_lsu_redemption_value)
        pub fn get_claimable_xrd(&self, receipt_id: NonFungibleLocalId) -> (Decimal, u64, Decimal, Decimal) {
            self.calculate_claimable_xrd(&receipt_id)
        }

        /// Gets a range of entries from the buy list order book.
        /// 
        /// This method returns a paginated view of the AVL tree buy list, useful for off-chain indexing
        /// or frontend displays. The buy list is ordered by discount (best first), then by epoch (oldest
        /// first), then by liquidity ID. This allows external systems to reconstruct the order matching
        /// priority without needing to query the entire tree at once.
        /// 
        /// # Arguments
        /// * `start_index`: The `u64` index to start from in the iteration
        /// * `count`: The `u64` maximum number of entries to return
        ///
        /// # Returns
        /// * A `Vec<(u128, NonFungibleGlobalId)>` containing tuples of AVL tree keys and receipt global IDs
        pub fn get_raw_buy_list_range(&self, start_index: u64, count: u64) -> Vec<(u128, NonFungibleGlobalId)>{
            let mut results = Vec::new();
            let mut current_index = 0u64;
            
            // Iterate through the AVL tree
            for (key, global_id, _) in self.buy_list.range(0..u128::MAX) {
                // Skip entries until we reach start_index
                if current_index < start_index {
                    current_index += 1;
                    continue;
                }
                
                // Stop if we've collected enough entries
                if results.len() >= count as usize {
                    break;
                }
                
                // Add the actual key and global_id
                results.push((key, global_id.clone()));
                
                current_index += 1;
            }
            
            results
        }

        pub fn get_active_liquidity_positions(&self, start_index: u64, count: u64) -> Vec<ReceiptDetailData> {
            let mut results = Vec::new();
            let mut current_index = 0u64;
            
            // Iterate through actual positions in the buy list
            for (_, global_id, _) in self.buy_list.range(0..u128::MAX) {
                if current_index < start_index {
                    current_index += 1;
                    continue;
                }
                
                if results.len() >= count as usize {
                    break;
                }
                
                let receipt_id = global_id.local_id().clone();
                results.push(self.get_receipt_detail(receipt_id));
                current_index += 1;
            }
            
            results
        }

        fn calculate_liquidity_ahead(&self, receipt_id: &NonFungibleLocalId) -> Decimal {
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), receipt_id.clone());
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(receipt_id);
            let discount = nft_data.discount;
            
            let mut liquidity_ahead = Decimal::ZERO;
            
            // Find our position's key in the tree
            let mut our_key = None;
            for (key, tree_global_id, _) in self.buy_list.range(0..u128::MAX) {
                if tree_global_id == global_id {
                    our_key = Some(key);
                    break;
                }
            }
            
            if let Some(our_key) = our_key {
                // Sum all liquidity from start up to (but not including) our position
                for (key, other_global_id, _) in self.buy_list.range(0..our_key) {
                    if let Some(kvs_data) = self.liquidity_data.get(&other_global_id) {
                        liquidity_ahead += kvs_data.xrd_liquidity_available;
                    }
                }
            }
            
            liquidity_ahead
        }

    }
    
}