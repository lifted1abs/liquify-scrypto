use scrypto::prelude::*;
use scrypto_avltree::AvlTree;

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
            set_component_status => restrict_to: [owner];
            set_platform_fee => restrict_to: [owner];
            set_automation_fee => restrict_to: [owner];
            collect_platform_fees => restrict_to: [owner];
            set_minimum_liquidity => restrict_to: [owner];
            set_receipt_image_url => restrict_to: [owner];
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
        receipt_image_url: Url,
        automation_fee: Decimal,
        automated_liquidity: KeyValueStore<u64, NonFungibleGlobalId>,
        automated_liquidity_index: u64,
    }

    impl Liquify {

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
                component_vaults: KeyValueStore::new(),
                liquidity_data: KeyValueStore::new(),
                liquidity_index,
                discounts,
                total_xrd_volume: Decimal::ZERO,
                total_xrd_locked: Decimal::ZERO,
                component_status: true,
                order_fill_counter: 1,
                platform_fee: dec!(0.00),
                fee_vault: Vault::new(XRD),
                minimum_liquidity: dec!(10000),
                receipt_image_url: Url::of("https://bafybeib7cokm27lwwkunaibn7hczijn3ztkypbzttmt7hymaov44s5e5sm.ipfs.w3s.link/liquify2.png"),
                automation_fee: dec!(5),
                automated_liquidity: KeyValueStore::new(),
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
                }
            })
            .globalize();

            (liquify_component, liquify_owner_badge)
        }

        pub fn add_liquidity(
            &mut self, 
            xrd_bucket: Bucket, 
            discount: Decimal, 
            auto_unstake: bool,
            auto_refill: bool,
            refill_threshold: Decimal
        ) -> NonFungibleBucket {
            
            assert!(self.component_status == true, "Liquify is not accepting new liquidity at this time.");
            assert!(xrd_bucket.resource_address() == XRD, "Bucket must contain XRD");
            assert!(xrd_bucket.amount() >= self.minimum_liquidity, "This amount is below the minimum liquidity requirement XRD");
            assert!(self.discounts.contains(&discount), "This discount % is not supported");
            
            if auto_refill {
                assert!(refill_threshold >= dec!(10000), "Refill threshold must be at least 10,000 XRD");
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
        
            self.buy_list.insert(combined_key.key, global_id);
        
            let index_usize = match (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>() {
                Ok(index) => index,
                Err(_) => panic!("Failed to calculate liquidity index for discount: {}", discount),
            };

            let currently_liquidity_at_discount = self.liquidity_index[index_usize];
            self.liquidity_index[index_usize] = currently_liquidity_at_discount + xrd_bucket.amount();
        
            self.total_xrd_locked += xrd_bucket.amount();
            self.xrd_liquidity.put(xrd_bucket);
        
            new_liquidity_receipt
        }

        pub fn increase_liquidity(&mut self, receipt_bucket: Bucket, xrd_bucket: Bucket) -> Bucket {
            assert!(receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt");
            assert!(receipt_bucket.amount() == dec!(1), "Must provide exactly one liquidity receipt");
            assert!(xrd_bucket.resource_address() == XRD, "Bucket must contain XRD");
            assert!(xrd_bucket.amount() >= self.minimum_liquidity, "This amount is below the minimum liquidity requirement");

            let local_id = receipt_bucket.as_non_fungible().non_fungible_local_id();
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), local_id.clone());
            let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
            
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
            kvs_data.xrd_liquidity_available += xrd_bucket.amount();
            let current_epoch = Runtime::current_epoch().number() as u32;
            kvs_data.last_added_epoch = current_epoch;
            
            // Create new key with current epoch (puts it at back of queue for same discount/epoch)
            let new_combined_key = CombinedKey::new(discount_u64, current_epoch, self.liquidity_receipt_counter);
            self.liquidity_receipt_counter += 1;
            
            // Reinsert at new position
            self.buy_list.insert(new_combined_key.key, global_id);
            
            // Update liquidity index
            let index_usize = (nft_data.discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
            self.liquidity_index[index_usize] += xrd_bucket.amount();
            
            self.total_xrd_locked += xrd_bucket.amount();
            self.xrd_liquidity.put(xrd_bucket);
            
            receipt_bucket
        }

        pub fn update_automation(
            &mut self, 
            receipt_bucket: Bucket, 
            auto_refill: bool, 
            refill_threshold: Decimal
        ) -> Bucket {
            assert!(receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt");
            assert!(receipt_bucket.amount() == dec!(1), "Must provide exactly one liquidity receipt");
            
            if auto_refill {
                assert!(refill_threshold >= dec!(10000), "Refill threshold must be at least 10,000 XRD");
            }

            let local_id = receipt_bucket.as_non_fungible().non_fungible_local_id();
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&local_id);
            
            // Can only automate receipts that have auto_unstake enabled
            if auto_refill {
                assert!(nft_data.auto_unstake, "Can only enable automation on receipts with auto_unstake enabled");
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
            
            receipt_bucket
        }



        pub fn cycle_liquidity(&mut self, receipt_id: NonFungibleLocalId) -> Bucket {
            let nft_data: LiquidityReceipt = self.liquidity_receipt.get_non_fungible_data(&receipt_id);
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), receipt_id.clone());
            
            // Get data, check conditions, then drop the borrow
            let (auto_refill, auto_unstake, refill_threshold, discount) = {
                let kvs_data = self.liquidity_data.get(&global_id).unwrap();
                (nft_data.auto_refill, nft_data.auto_unstake, nft_data.refill_threshold, nft_data.discount)
            };
            
            assert!(auto_refill, "Automation not enabled for this receipt");
            assert!(auto_unstake, "Can only cycle receipts with auto_unstake enabled");
            
            // Calculate claimable XRD from fills
            let claimable_xrd = self.calculate_claimable_xrd(&receipt_id);
            assert!(claimable_xrd >= refill_threshold, "Not enough claimable XRD to meet threshold");
            
            // Collect all fills for this receipt
            let mut total_xrd = Bucket::new(XRD);
            let receipt_id_u64 = match receipt_id.clone() {
                NonFungibleLocalId::Integer(i) => i.value(),
                _ => panic!("Invalid NFT ID type")
            };
            
            // Process all fills for this receipt
            let start_key = CombinedKey::new(receipt_id_u64, 1, 0).key;
            let end_key = CombinedKey::new(receipt_id_u64, u32::MAX, 0).key;
            
            // Collect keys and data first, then process
            let mut fills_to_process = Vec::new();
            for (key, value, _) in self.order_fill_tree.range(start_key..=end_key) {
                fills_to_process.push((key, value.clone()));
            }
            
            // Now process the fills
            for (avl_key, unstake_nft_or_lsu) in fills_to_process {
                match unstake_nft_or_lsu {
                    UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                        // Process unstake NFT - claim the XRD
                        let unstake_nft_vault = self.component_vaults.get(&unstake_nft_data.resource_address).unwrap();
                        let unstake_nft = unstake_nft_vault.as_non_fungible().take_non_fungible(&unstake_nft_data.id);
                        
                        // Get validator and claim
                        let validator_address = self.get_validator_from_unstake_nft(&unstake_nft_data.resource_address);
                        let mut validator: Global<Validator> = Global::from(validator_address);
                        let claimed_xrd = validator.claim_xrd(unstake_nft.into());
                        total_xrd.put(claimed_xrd.into());
                    }
                    UnstakeNFTOrLSU::LSU(_) => {
                        panic!("Cannot cycle LSU fills - receipt must have auto_unstake enabled");
                    }
                }
                
                // Remove the processed fill
                self.order_fill_tree.remove(&avl_key);
            }
            
            // Update KVS data
            let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
            kvs_data.fills_to_collect = 0;
            
            // Take automation fee
            let fee_amount = self.automation_fee;
            let automation_fee_bucket = total_xrd.take(fee_amount);
            self.fee_vault.put(automation_fee_bucket);
            
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
            
            // Put XRD in vault
            self.xrd_liquidity.put(total_xrd);
            self.total_xrd_locked += xrd_to_add;
            
            // Return empty bucket as confirmation
            Bucket::new(XRD)
        }

        pub fn get_claimable_xrd(&self, receipt_id: NonFungibleLocalId) -> Decimal {
            self.calculate_claimable_xrd(&receipt_id)
        }

        pub fn get_liquidity_data(&self, receipt_id: NonFungibleLocalId) -> LiquidityData {
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), receipt_id);
            self.liquidity_data.get(&global_id).unwrap().clone()
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
                        let nft_manager = ResourceManager::from(unstake_nft_data.resource_address);
                        
                        // Get claim_amount and claim_epoch from metadata
                        let claim_amount: Decimal = nft_manager
                            .get_metadata("claim_amount")
                            .unwrap()
                            .unwrap_or_else(|| Runtime::panic(String::from("Invalid stake claim NFT - no claim_amount")));
                        
                        let claim_epoch: u64 = nft_manager
                            .get_metadata("claim_epoch")
                            .unwrap()
                            .unwrap_or_else(|| Runtime::panic(String::from("Invalid stake claim NFT - no claim_epoch")));
                        
                        // Check if past the unbonding period
                        let current_epoch = Runtime::current_epoch().number();
                        if current_epoch >= claim_epoch {
                            total_claimable += claim_amount;
                        }
                    }
                    UnstakeNFTOrLSU::LSU(_) => {
                        // LSUs are NOT claimable for XRD - they need to be collected first
                        // They contribute 0 to claimable XRD
                    }
                }
            }
            
            total_claimable
        }

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
            }
        
            let user_funds = self.xrd_liquidity.take(total_order_size);
            self.total_xrd_locked -= total_order_size;
        
            (user_funds, liquidity_receipt_bucket)
        }

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

        pub fn liquify_unstake_off_ledger(&mut self, lsu_bucket: FungibleBucket, order_keys: Vec<u128>) -> (Bucket, FungibleBucket) {
            assert!(self.validate_lsu(lsu_bucket.resource_address()), "Bucket must contain a native Radix Validator LSU");
            self.process_unstake(lsu_bucket, order_keys)
        }

        fn process_unstake(&mut self, mut lsu_bucket: FungibleBucket, order_keys: Vec<u128>) -> (Bucket, FungibleBucket) {
            let mut xrd_bucket: Bucket = Bucket::new(XRD);
            let mut validator = self.get_validator_from_lsu(lsu_bucket.resource_address());
            
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
                
                kvs_updates.push((global_id, new_xrd_available, fill_amount, current_fills + 1));
                
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
            
            (xrd_bucket, lsu_bucket)
        }
        
        pub fn collect_fills(&mut self, liquidity_receipt_bucket: Bucket, number_of_fills_to_collect: u64) -> (Vec<Bucket>, Bucket) {
            
            assert!(
                liquidity_receipt_bucket.resource_address() == self.liquidity_receipt.address(),
                "Bucket must contain Liquify liquidity receipts NFT(s)"
            );

            let mut bucket_vec: Vec<Bucket> = Vec::new();
            let mut collect_counter: u64 = 0;
            let mut all_updates = vec![];

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
                            bucket_vec.push(lsu_bucket);
                        }

                        UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                            let mut unstake_nft_bucket = Bucket::new(unstake_nft_data.resource_address);
                            let unstake_nft_id = &unstake_nft_data.id;
                            let unstake_nft_vault = self.component_vaults.get_mut(&unstake_nft_data.resource_address).unwrap();
                            unstake_nft_bucket.put(unstake_nft_vault.as_non_fungible().take_non_fungible(&unstake_nft_id).into());
                            bucket_vec.push(unstake_nft_bucket);
                        }
                    }

                    self.order_fill_tree.remove(&avl_key);
                }

                let new_fills_to_collect = fills_to_collect - fills_collected_for_this_order;
                all_updates.push((global_id, new_fills_to_collect));
            }

            // Apply all KVS updates
            for (global_id, new_fills_to_collect) in all_updates {
                let mut kvs_data = self.liquidity_data.get_mut(&global_id).unwrap();
                kvs_data.fills_to_collect = new_fills_to_collect;
            }

            (bucket_vec, liquidity_receipt_bucket)
        }
        
        pub fn collect_platform_fees(&mut self) -> Bucket {
            self.fee_vault.take_all()
        }
        
        pub fn set_component_status(&mut self, status: bool) {
            self.component_status = status;
        }

        pub fn set_platform_fee(&mut self, fee: Decimal) {
            self.platform_fee = fee;
        }

        pub fn set_automation_fee(&mut self, new_fee: Decimal) {
            assert!(new_fee >= dec!(0), "Automation fee cannot be negative");
            self.automation_fee = new_fee;
        }

        pub fn set_minimum_liquidity(&mut self, min: Decimal) {
            self.minimum_liquidity = min;
        }

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
    }
}