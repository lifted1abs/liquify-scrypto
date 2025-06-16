// src/liquify.rs

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
    pub fills_to_collect: u64,
    pub last_added_epoch: u32,
    pub claimable_xrd: Decimal,
}

// 2. Automation Ready Receipts Getter
#[derive(ScryptoSbor, Debug, Clone)]
pub struct AutomationReadyReceipt {
    pub receipt_id: NonFungibleLocalId,
    pub claimable_xrd: Decimal,
    pub refill_threshold: Decimal,
    pub xrd_after_fee: Decimal,
}

// NFT - Data that doesn't change during fills
#[derive(NonFungibleData, ScryptoSbor, PartialEq, Debug, Clone)]
pub struct LiquidityReceipt {
    key_image_url: Url,
    discount: Decimal,
    auto_unstake: bool,
    #[mutable]
    auto_refill: bool,
    #[mutable]
    refill_threshold: Decimal,
}

// KVS - Data that updates during fills
#[derive(ScryptoSbor, PartialEq, Debug, Clone)]
pub struct LiquidityData {
    xrd_liquidity_filled: Decimal,
    xrd_liquidity_available: Decimal,
    fills_to_collect: u64,
    last_added_epoch: u32,
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
struct AutomationUpdatedEvent {
    receipt_id: NonFungibleLocalId,
    auto_refill: bool,
    refill_threshold: Decimal,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct CombinedKey {
    key: u128,
}

impl CombinedKey {
    pub fn new(discount_u64: u64, epoch: u32, liquidity_id: u64) -> Self {
        // Pack: discount (16 bits) | epoch (32 bits) | liquidity_id (64 bits)
        let key = ((discount_u64 as u128) << 96) | ((epoch as u128) << 64) | (liquidity_id as u128);
        CombinedKey { key }
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
    AutomationUpdatedEvent
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
            update_automation => PUBLIC;
            cycle_liquidity => PUBLIC;
            get_claimable_xrd => PUBLIC;
            get_liquidity_data => PUBLIC;
            get_buy_list_range => PUBLIC;
            get_liquidity_data_range => PUBLIC;
            get_automated_liquidity_range => PUBLIC;
            get_automation_ready_receipts => PUBLIC;
            get_receipt_detail => PUBLIC;
            set_component_status => restrict_to: [owner];
            set_platform_fee => restrict_to: [owner];
            set_automation_fee => restrict_to: [owner];
            collect_platform_fees => restrict_to: [owner];
            set_minimum_liquidity => restrict_to: [owner];
            set_receipt_image_url => restrict_to: [owner];
            set_minimum_refill_threshold => restrict_to: [owner];
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
        liquidity_index: Vec<Decimal>,
        discounts: Vec<Decimal>,
        platform_fee: Decimal,
        fee_vault: Vault,
        minimum_liquidity: Decimal,
        minimum_refill_threshold: Decimal,
        receipt_image_url: Url,
        automation_fee: Decimal,
        automated_liquidity: KeyValueStore<u64, NonFungibleGlobalId>,
        automated_liquidity_index: u64,
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
                buy_list: AvlTree::new(),
                order_fill_tree: AvlTree::new(),
                component_vaults: KeyValueStore::new_with_registered_type(),
                liquidity_data: KeyValueStore::new_with_registered_type(),
                liquidity_index,
                discounts,
                total_xrd_volume: Decimal::ZERO,
                total_xrd_locked: Decimal::ZERO,
                component_status: false,  // CHANGED: Start in disabled state
                order_fill_counter: 1,
                platform_fee: dec!(0.00),
                fee_vault: Vault::new(XRD),
                minimum_liquidity: dec!(10000),
                minimum_refill_threshold: dec!(10000),
                receipt_image_url: Url::of("https://bafybeib7cokm27lwwkunaibn7hczijn3ztkypbzttmt7hymaov44s5e5sm.ipfs.w3s.link/liquify2.png"),
                automation_fee: dec!(5),
                automated_liquidity: KeyValueStore::new_with_registered_type(),
                automated_liquidity_index: 1,
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
                    update_automation => Free, updatable;
                    cycle_liquidity => Free, updatable;
                    get_claimable_xrd => Free, updatable;
                    get_liquidity_data => Free, updatable;
                    set_component_status => Free, updatable;
                    set_platform_fee => Free, updatable;
                    set_automation_fee => Free, updatable;
                    collect_platform_fees => Free, updatable;
                    set_minimum_liquidity => Free, updatable;
                    set_receipt_image_url => Free, updatable;
                    get_buy_list_range => Free, updatable;
                    get_liquidity_data_range => Free, updatable;
                    get_automated_liquidity_range => Free, updatable;
                    set_minimum_refill_threshold => Free, updatable;
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
        pub fn add_liquidity(&mut self, xrd_bucket: Bucket, discount: Decimal, auto_unstake: bool, auto_refill: bool, refill_threshold: Decimal) -> NonFungibleBucket {
            
            assert!(self.component_status == true, "Liquify is not accepting new liquidity at this time.");
            assert!(xrd_bucket.resource_address() == XRD, "Bucket must contain XRD");
            assert!(xrd_bucket.amount() >= self.minimum_liquidity, "This amount is below the minimum liquidity requirement XRD");
            assert!(self.discounts.contains(&discount), "This discount % is not supported");
            
            // ADDED: Validate auto_refill requires auto_unstake
            if auto_refill {
                assert!(auto_unstake, "Auto refill can only be enabled when auto unstake is enabled");
                assert!(refill_threshold >= self.minimum_refill_threshold, "Refill threshold is below required minimum");
            }
        
            let discount_u64 = (discount * dec!(10000)).checked_floor().unwrap().to_string().parse::<u64>().unwrap();
            let current_epoch = Runtime::current_epoch().number() as u32;
            let combined_key = CombinedKey::new(discount_u64, current_epoch, self.liquidity_receipt_counter);
            let id = NonFungibleLocalId::Integer(IntegerNonFungibleLocalId::new(self.liquidity_receipt_counter));

            // Mint NFT with immutable + automation data
            let liquidity_receipt_data = LiquidityReceipt {
                key_image_url: self.receipt_image_url.clone(),
                discount,
                auto_unstake,
                auto_refill,
                refill_threshold,
            };
        
            let new_liquidity_receipt: NonFungibleBucket = self.liquidity_receipt.mint_non_fungible(&id, liquidity_receipt_data);
            
            // Store mutable data in KVS
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), id);
            let liquidity_data = LiquidityData {
                xrd_liquidity_filled: dec!(0),
                xrd_liquidity_available: xrd_bucket.amount(),
                fills_to_collect: 0,
                last_added_epoch: current_epoch,
            };
            self.liquidity_data.insert(global_id.clone(), liquidity_data);
            
            // Add to automated tracking if auto_refill is enabled
            if auto_refill {
                self.automated_liquidity.insert(self.automated_liquidity_index, global_id.clone());
                self.automated_liquidity_index += 1;
            }
            
            self.liquidity_receipt_counter += 1;
        
            self.buy_list.insert(combined_key.key, global_id.clone());
        
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
            
            // Get current discount
            let discount_u64 = (nft_data.discount * dec!(10000)).checked_floor().unwrap().to_string().parse::<u64>().unwrap();
            
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
            
            // Create new key with current epoch (puts it at back of queue for same discount/epoch)
            let new_combined_key = CombinedKey::new(discount_u64, current_epoch, self.liquidity_receipt_counter);
            self.liquidity_receipt_counter += 1;
            
            // Reinsert at new position
            self.buy_list.insert(new_combined_key.key, global_id.clone());
            
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

        /// Updates automation settings for an existing liquidity position.
        /// 
        /// This method allows liquidity providers to enable or disable automation features on their receipt.
        /// Auto refill can only be enabled on receipts that have auto unstake enabled. When auto refill is
        /// enabled, the position is tracked in the automated liquidity index. When disabled, it is removed
        /// from tracking. The refill threshold determines the minimum XRD amount from collected fills needed
        /// to trigger an automatic liquidity refill cycle.
        /// 
        /// # Arguments
        /// * `receipt_bucket`: A `Bucket` containing exactly one liquidity receipt NFT
        /// * `auto_refill`: A `bool` indicating whether to enable automatic liquidity refilling
        /// * `refill_threshold`: A `Decimal` representing the minimum XRD amount to trigger refill (minimum 10,000)
        ///
        /// # Returns
        /// * A `Bucket` containing the same liquidity receipt NFT with updated automation settings
        pub fn update_automation(&mut self, receipt_bucket: Bucket, auto_refill: bool, refill_threshold: Decimal) -> Bucket {
            assert!(receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt");
            assert!(receipt_bucket.amount() == dec!(1), "Must provide exactly one liquidity receipt");
            
            let local_id = receipt_bucket.as_non_fungible().non_fungible_local_id();
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
            let xrd_liquidity_available = self.liquidity_data.get(&NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone())).unwrap().xrd_liquidity_available;

            assert!(refill_threshold <= xrd_liquidity_available, "Refill threshold cannot exceed available liquidity");
            
            // ENHANCED VALIDATION
            if auto_refill {
                assert!(nft_data.auto_unstake, "Cannot enable auto refill on a receipt that has auto unstake disabled");
                assert!(refill_threshold >= dec!(10000), "Refill threshold must be at least 10,000 XRD");
            }
            
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
            
            // Handle automation tracking changes
            if auto_refill && !nft_data.auto_refill {
                // Enabling automation
                self.automated_liquidity.insert(self.automated_liquidity_index, global_id.clone());
                self.automated_liquidity_index += 1;
            } else if !auto_refill && nft_data.auto_refill {
                // Disabling automation - need to find and remove
                let mut target_index = None;
                for i in 1..self.automated_liquidity_index {
                    if let Some(stored_global_id) = self.automated_liquidity.get(&i) {
                        if *stored_global_id == global_id {
                            target_index = Some(i);
                            break;
                        }
                    }
                }
                
                if let Some(index_to_remove) = target_index {
                    self.automated_liquidity.remove(&index_to_remove);
                    
                    // Move last entry to fill the gap (if not removing the last entry)
                    let last_index = self.automated_liquidity_index - 1;
                    if index_to_remove != last_index && last_index > 0 {
                        if let Some(last_entry) = self.automated_liquidity.get(&last_index) {
                            let last_entry_clone = (*last_entry).clone();
                            self.automated_liquidity.remove(&last_index);
                            self.automated_liquidity.insert(index_to_remove, last_entry_clone);
                        }
                    }
                    
                    self.automated_liquidity_index -= 1;
                }
            }
            
            // Update NFT data
            self.liquidity_receipt.update_non_fungible_data(&local_id, "auto_refill", auto_refill);
            self.liquidity_receipt.update_non_fungible_data(&local_id, "refill_threshold", refill_threshold);
            
            // Emit the automation update event
            Runtime::emit_event(AutomationUpdatedEvent {
                receipt_id: local_id,
                auto_refill,
                refill_threshold,
            });
            
            receipt_bucket
        }


































        /// Cycles liquidity by claiming fills and re-adding as liquidity.
        /// 
        /// This method allows automated systems or users to cycle a liquidity position by claiming all available
        /// XRD from unstake NFT fills and automatically re-adding it as liquidity. Only works for receipts with
        /// both auto_unstake and auto_refill enabled. The position must have enough claimable XRD to meet the
        /// refill threshold. An automation fee is deducted and returned to the caller as payment for executing
        /// the cycle. The remaining XRD is added back to the position's available liquidity.
        /// 
        /// # Arguments
        /// * `receipt_id`: The `NonFungibleLocalId` of the liquidity receipt to cycle
        ///
        /// # Returns
        /// * A `Bucket` containing the automation fee amount in XRD as payment to the caller

        
        pub fn cycle_liquidity(&mut self, receipt_id: NonFungibleLocalId, max_fills_to_process: u64) -> FungibleBucket {
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&receipt_id);
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), receipt_id.clone());
            
            // Get data, check conditions, then drop the borrow
            let (auto_refill, auto_unstake, refill_threshold, discount) = {
                let kvs_data = self.liquidity_data.get(&global_id).unwrap();
                (nft_data.auto_refill, nft_data.auto_unstake, nft_data.refill_threshold, nft_data.discount)
            };
            
            assert!(auto_refill, "Automation not enabled for this receipt");
            assert!(auto_unstake, "Can only cycle receipts with auto_unstake enabled");
            assert!(max_fills_to_process > 0, "Must process at least one fill");
            
            // Calculate claimable XRD from fills (limited by max_fills_to_process)
            let claimable_xrd = self.calculate_limited_claimable_xrd(&receipt_id, max_fills_to_process);
            assert!(claimable_xrd >= refill_threshold, "Not enough claimable XRD to meet threshold");
            
            // Collect fills for this receipt (limited by max_fills_to_process)
            let receipt_id_u64 = match receipt_id.clone() {
                NonFungibleLocalId::Integer(i) => i.value(),
                _ => panic!("Invalid NFT ID type")
            };
            
            // Process fills for this receipt
            let start_key = CombinedKey::new(receipt_id_u64, 1, 0).key;
            let end_key = CombinedKey::new(receipt_id_u64, u32::MAX, 0).key;
            
            // Collect keys and data first, then process (limited by max_fills_to_process)
            let mut fills_to_process = Vec::new();
            let mut fills_collected = 0u64;
            
            for (key, value, _) in self.order_fill_tree.range(start_key..=end_key) {
                if fills_collected >= max_fills_to_process {
                    break;
                }
                fills_to_process.push((key, value.clone()));
                fills_collected += 1;
            }
            
            // Process the fills - collect all XRD buckets first
            let mut xrd_buckets = Vec::new();
            let mut processed_keys = Vec::new();
            
                for (avl_key, unstake_nft_or_lsu) in fills_to_process {
                    match unstake_nft_or_lsu {
                        UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                            let local_id: NonFungibleLocalId = unstake_nft_data.id.clone();
                            
                            // Check if NFT is ready to claim
                            let nft_data: UnstakeData = NonFungibleResourceManager::from(unstake_nft_data.resource_address)
                                .get_non_fungible_data(&local_id);
                            let current_epoch = Runtime::current_epoch();
                            
                            if current_epoch >= nft_data.claim_epoch {
                                // Get validator address first (before taking mutable borrow)
                                let validator_address = self.get_validator_from_unstake_nft(&unstake_nft_data.resource_address);
                                let mut validator: Global<Validator> = Global::from(validator_address);
                                
                                // NFT is ready to claim
                                let unstake_nft_vault = self.component_vaults.get_mut(&unstake_nft_data.resource_address).unwrap();
                                let unstake_nft = unstake_nft_vault.as_non_fungible().take_non_fungible(&local_id);
                                // Drop the mutable borrow by ending the scope
                                drop(unstake_nft_vault);
                                
                                // Claim XRD and store bucket
                                let claimed_xrd_bucket: FungibleBucket = validator.claim_xrd(unstake_nft);
                                xrd_buckets.push(claimed_xrd_bucket);
                                processed_keys.push(avl_key);
                            }
                            // If not ready, don't process or remove
                        }
                        UnstakeNFTOrLSU::LSU(_) => {
                            panic!("Cannot cycle LSU fills - receipt must have auto_unstake enabled");
                        }
                    }
                }
            
            // Save processed count before moving processed_keys
            let processed_count = processed_keys.len() as u64;
            
            // Remove processed fills from tree
            for key in processed_keys {
                self.order_fill_tree.remove(&key);
            }
            
            // Combine all XRD buckets
            let mut total_xrd = FungibleBucket::new(XRD);
            for xrd_bucket in xrd_buckets {
                total_xrd.put(xrd_bucket);
            }
            
            // Update KVS data
            let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
            kvs_data.fills_to_collect = kvs_data.fills_to_collect.saturating_sub(processed_count);
            
            // Take automation fee
            let automation_fee_bucket = total_xrd.take(self.automation_fee);
            
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
            
            // Create new key with current epoch
            let discount_u64 = (discount * dec!(10000)).checked_floor().unwrap().to_string().parse::<u64>().unwrap();
            let new_combined_key = CombinedKey::new(discount_u64, current_epoch, self.liquidity_receipt_counter);
            self.liquidity_receipt_counter += 1;
            
            // Reinsert at new position
            self.buy_list.insert(new_combined_key.key, global_id);
            
            // Update liquidity index
            let index_usize = (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
            self.liquidity_index[index_usize] += xrd_to_add;
            
            // Put remaining XRD in vault
            self.xrd_liquidity.put(total_xrd.into());
            self.total_xrd_locked += xrd_to_add;
            
            // Emit the cycle event
            Runtime::emit_event(LiquidityCycledEvent {
                receipt_id,
                xrd_amount_cycled: xrd_to_add,
                automation_fee: self.automation_fee,
            });
            
            // Return the automation fee to the caller
            automation_fee_bucket
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

            for local_id in liquidity_receipt_bucket.as_non_fungible().non_fungible_local_ids() {
                let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
                let kvs_data = self.liquidity_data.get(&global_id).unwrap();
                assert!(kvs_data.xrd_liquidity_available > dec!(0), "No liquidity available to remove");
            }

            let mut total_order_size: Decimal = Decimal::ZERO;

            for local_id in liquidity_receipt_bucket.as_non_fungible().non_fungible_local_ids() {
                let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
                let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
                let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
                let discount = nft_data.discount;
                let order_size = kvs_data.xrd_liquidity_available;

                // If this order was automated, disable automation and remove from tracking
                if nft_data.auto_refill {
                    self.liquidity_receipt.update_non_fungible_data(&local_id, "auto_refill", false);
                    
                    // Find and remove from automated tracking
                    let mut target_index = None;
                    for i in 1..self.automated_liquidity_index {
                        if let Some(stored_global_id) = self.automated_liquidity.get(&i) {
                            if *stored_global_id == global_id {
                                target_index = Some(i);
                                break;
                            }
                        }
                    }
                    
                    if let Some(index_to_remove) = target_index {
                        self.automated_liquidity.remove(&index_to_remove);
                        
                        // Move last entry to fill the gap (if not removing the last entry)
                        let last_index = self.automated_liquidity_index - 1;
                        if index_to_remove != last_index && last_index > 0 {
                            if let Some(last_entry) = self.automated_liquidity.get(&last_index) {
                                let last_entry_clone = (*last_entry).clone();
                                self.automated_liquidity.remove(&last_index);
                                self.automated_liquidity.insert(index_to_remove, last_entry_clone);
                            }
                        }
                        
                        self.automated_liquidity_index -= 1;
                    }
                }

                let index = (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
                let currently_liquidity_at_discount = self.liquidity_index[index];
                self.liquidity_index[index] = currently_liquidity_at_discount - order_size;

                total_order_size += order_size;

                // Find and remove from AVL tree
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
                kvs_data.xrd_liquidity_available = dec!(0);
                
                // Emit the event for this receipt
                Runtime::emit_event(LiquidityRemovedEvent {
                    receipt_id: local_id,
                    xrd_amount: order_size,
                });
            }

            let user_funds = self.xrd_liquidity.take(total_order_size);
            self.total_xrd_locked -= total_order_size;

            (user_funds, liquidity_receipt_bucket)
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

            let mut order_keys = Vec::new();
            let mut iter_count = 0;
            
            self.buy_list.range_mut(0..u128::MAX).for_each(|(avl_key, _global_id, _)| {
                if iter_count >= max_iterations as usize {
                    return scrypto_avltree::IterMutControl::Break;
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
            self.process_unstake(lsu_bucket, order_keys)
        }

        fn process_unstake(&mut self, mut lsu_bucket: FungibleBucket, order_keys: Vec<u128>) -> (Bucket, FungibleBucket) {
            let mut xrd_bucket: Bucket = Bucket::new(XRD);
            let mut validator = self.get_validator_from_lsu(lsu_bucket.resource_address());
            
            // Store initial values for event
            let lsu_resource = lsu_bucket.resource_address();
            let initial_lsu_amount = lsu_bucket.amount();
            
            // Pre-calculate redemption rate
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

                // Queue fill operation
                let order_fill_key = CombinedKey::new(local_id_u64, self.order_fill_counter as u32, 0).key;
                self.order_fill_counter += 1;
                
                if auto_unstake {
                    unstake_operations.push((order_fill_key, lsu_taken));
                    // We'll get the vault resource after unstaking
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
            let mut event_data_per_receipt: std::collections::HashMap<NonFungibleLocalId, (u64, Vec<(Decimal, ResourceAddress)>, Vec<NonFungibleGlobalId>)> = std::collections::HashMap::new();

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

                let start_key = CombinedKey::new(order_id_u64, 1, 0).key;
                let end_key = CombinedKey::new(order_id_u64, u32::MAX, 0).key;

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

        /// Sets the automation fee amount.
        /// 
        /// This method allows the owner to adjust the fee paid to callers who execute cycle_liquidity for
        /// automated positions. The fee incentivizes external actors to monitor and cycle positions when
        /// they reach their refill thresholds. The fee is paid from the claimed XRD before re-adding
        /// liquidity. Only the holder of the owner badge can call this method.
        /// 
        /// # Arguments
        /// * `new_fee`: A `Decimal` representing the fixed XRD amount paid per cycle operation
        ///
        /// # Returns
        /// * None
        pub fn set_automation_fee(&mut self, new_fee: Decimal) {
            assert!(new_fee >= dec!(0), "Automation fee cannot be negative");
            self.automation_fee = new_fee;
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

        fn ensure_user_vault_exists(&mut self, resource: ResourceAddress) {
            if !self.component_vaults.get(&resource).is_some() {
                let new_vault = Vault::new(resource);
                self.component_vaults.insert(resource, new_vault);
            }
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
            let kvs_data = self.liquidity_data.get(&global_id).unwrap();
            
            let claimable_xrd = if nft_data.auto_unstake {
                self.calculate_claimable_xrd(&receipt_id)
            } else {
                dec!(0)
            };
            
            ReceiptDetailData {
                receipt_id,
                discount: nft_data.discount,
                auto_unstake: nft_data.auto_unstake,
                auto_refill: nft_data.auto_refill,
                refill_threshold: nft_data.refill_threshold,
                xrd_liquidity_available: kvs_data.xrd_liquidity_available,
                xrd_liquidity_filled: kvs_data.xrd_liquidity_filled,
                fills_to_collect: kvs_data.fills_to_collect,
                last_added_epoch: kvs_data.last_added_epoch,
                claimable_xrd,
            }
        }
        
        pub fn get_automation_ready_receipts(&self) -> Vec<AutomationReadyReceipt> {
            let mut ready_receipts = Vec::new();
            
            // Iterate through automated_liquidity KVS
            for i in 1..self.automated_liquidity_index {
                if let Some(global_id) = self.automated_liquidity.get(&i) {
                    let receipt_id = global_id.local_id().clone();
                    let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&receipt_id);
                    
                    // Calculate claimable XRD
                    let claimable_xrd = self.calculate_claimable_xrd(&receipt_id);
                    
                    // Only include if claimable meets or exceeds threshold
                    if claimable_xrd >= nft_data.refill_threshold {
                        let xrd_after_fee = claimable_xrd - self.automation_fee;
                        
                        ready_receipts.push(AutomationReadyReceipt {
                            receipt_id,
                            claimable_xrd,
                            refill_threshold: nft_data.refill_threshold,
                            xrd_after_fee,
                        });
                    }
                }
            }
            
            ready_receipts
        }

        fn calculate_limited_claimable_xrd(&self, receipt_id: &NonFungibleLocalId, max_fills: u64) -> Decimal {
            let receipt_id_u64 = match receipt_id {
                NonFungibleLocalId::Integer(i) => i.value(),
                _ => return dec!(0)
            };
            
            let start_key = CombinedKey::new(receipt_id_u64, 1, 0).key;
            let end_key = CombinedKey::new(receipt_id_u64, u32::MAX, 0).key;
            
            let mut total_claimable = dec!(0);
            let mut fills_checked = 0u64;
            
            for (_, unstake_nft_or_lsu, _) in self.order_fill_tree.range(start_key..=end_key) {
                if fills_checked >= max_fills {
                    break;
                }
                
                match unstake_nft_or_lsu {
                    UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                        let nft_data: UnstakeData = NonFungibleResourceManager::from(unstake_nft_data.resource_address)
                            .get_non_fungible_data(&unstake_nft_data.id);
                        
                        // Check if past the unbonding period
                        let current_epoch = Runtime::current_epoch();
                        if current_epoch >= nft_data.claim_epoch {
                            total_claimable += nft_data.claim_amount;
                        }
                    }
                    UnstakeNFTOrLSU::LSU(_) => {
                        // LSUs are NOT claimable for XRD - they need to be collected first
                    }
                }
                
                fills_checked += 1;
            }
            
            total_claimable
        }

        fn calculate_claimable_xrd(&self, receipt_id: &NonFungibleLocalId) -> Decimal {
            let receipt_id_u64 = match receipt_id {
                NonFungibleLocalId::Integer(i) => i.value(),
                _ => return dec!(0)
            };
            
            let start_key = CombinedKey::new(receipt_id_u64, 1, 0).key;
            let end_key = CombinedKey::new(receipt_id_u64, u32::MAX, 0).key;
            
            let mut total_claimable = dec!(0);
            
            for (_, unstake_nft_or_lsu, _) in self.order_fill_tree.range(start_key..=end_key) {
                match unstake_nft_or_lsu {
                    UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                        let nft_data: UnstakeData = NonFungibleResourceManager::from(unstake_nft_data.resource_address)
                            .get_non_fungible_data(&unstake_nft_data.id);
                        
                        // Check if past the unbonding period
                        let current_epoch = Runtime::current_epoch();
                        if current_epoch >= nft_data.claim_epoch {
                            total_claimable += nft_data.claim_amount;
                        }
                    }
                    UnstakeNFTOrLSU::LSU(_) => {
                        // LSUs are NOT claimable for XRD - they need to be collected first
                    }
                }
            }
            
            total_claimable
        }

        /// Gets the claimable XRD amount for a liquidity position.
        /// 
        /// This method calculates how much XRD can be claimed from unstake NFTs that have passed their
        /// unbonding period. It only counts NFTs where the claim epoch has been reached - LSU fills are
        /// not counted as they need to be collected first. This is useful for checking if a position
        /// meets the refill threshold for automation or for users to see their claimable amounts.
        /// 
        /// # Arguments
        /// * `receipt_id`: The `NonFungibleLocalId` of the liquidity receipt to check
        ///
        /// # Returns
        /// * A `Decimal` representing the total XRD amount claimable from matured unstake NFTs

        pub fn get_claimable_xrd(&self, receipt_id: NonFungibleLocalId) -> Decimal {
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
        pub fn get_buy_list_range(&self, start_index: u64, count: u64) -> Vec<(u128, NonFungibleGlobalId)>{
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

        /// Gets a range of liquidity data entries.
        /// 
        /// This method returns paginated liquidity position data by iterating through sequential receipt IDs
        /// starting from the given index. Useful for indexers or dashboards that need to display all active
        /// liquidity positions with their current state including available/filled amounts and pending fills.
        /// Note that receipt IDs start at 1, so start_index 0 will begin with receipt ID 1.
        /// 
        /// # Arguments
        /// * `start_index`: The `u64` starting position (0-based, maps to receipt ID start_index + 1)
        /// * `count`: The `u64` maximum number of entries to return
        ///
        /// # Returns
        /// * A `Vec<(NonFungibleGlobalId, LiquidityData)>` containing receipt IDs and their associated data
        pub fn get_liquidity_data_range(&self, start_index: u64, count: u64) -> Vec<(NonFungibleGlobalId, LiquidityData)> {
            let mut results = Vec::new();
            
            // Since liquidity_data is keyed by NonFungibleGlobalId, we'll iterate through receipt IDs
            // starting from start_index + 1 (since receipt counter starts at 1)
            let start_id = start_index + 1;
            let end_id = std::cmp::min(start_id + count, self.liquidity_receipt_counter);
            
            for id in start_id..end_id {
                let local_id = NonFungibleLocalId::Integer(IntegerNonFungibleLocalId::new(id));
                let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id);
                
                // Check if this global_id exists in our KVS
                if let Some(liquidity_data) = self.liquidity_data.get(&global_id) {
                    results.push((global_id, liquidity_data.clone()));
                }
            }
            
            results
        }

        /// Gets a range of automated liquidity positions.
        /// 
        /// This method returns paginated entries from the automated liquidity tracking index. Only positions
        /// with auto_refill enabled are included. The index maintains insertion order and handles gaps when
        /// positions disable automation. Useful for automation bots to identify which positions need cycling
        /// based on their refill thresholds and claimable amounts.
        /// 
        /// # Arguments
        /// * `start_index`: The `u64` index to start from (minimum 1)
        /// * `count`: The `u64` maximum number of entries to return
        ///
        /// # Returns
        /// * A `Vec<(u64, NonFungibleGlobalId)>` containing index positions and receipt global IDs
        pub fn get_automated_liquidity_range(&self, start_index: u64, count: u64) -> Vec<(u64, NonFungibleGlobalId)> {
            let mut results = Vec::new();
            
            // automated_liquidity is indexed from 1 to automated_liquidity_index - 1
            let start = std::cmp::max(start_index, 1);
            let end = std::cmp::min(start + count, self.automated_liquidity_index);
            
            for index in start..end {
                if let Some(global_id) = self.automated_liquidity.get(&index) {
                    results.push((index, global_id.clone()));
                }
            }
            
            results
        }

        /// Gets the current liquidity data for a specific receipt.
        /// 
        /// This method returns the mutable liquidity data stored in the key-value store for a given
        /// receipt ID. This includes the current available liquidity, amount already filled, number
        /// of fills pending collection, and the epoch when liquidity was last added. Useful for
        /// frontends to display position details or for users to check their liquidity status.
        /// 
        /// # Arguments
        /// * `receipt_id`: The `NonFungibleLocalId` of the liquidity receipt to query
        ///
        /// # Returns
        /// * A `LiquidityData` struct containing the current state of the liquidity position
        pub fn get_liquidity_data(&self, receipt_id: NonFungibleLocalId) -> LiquidityData {
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), receipt_id);
            self.liquidity_data.get(&global_id).unwrap().clone()
        }

    }
    
}