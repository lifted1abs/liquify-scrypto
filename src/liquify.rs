use scrypto::prelude::*;
use scrypto_avltree::AvlTree;

#[derive(NonFungibleData, ScryptoSbor, PartialEq, Debug, Clone)]
pub struct LiquidityDetails {
    key_image_url: Url,
    #[mutable]
    liquidity_status: LiquidityStatus,
    total_xrd_amount: Decimal,
    discount: Decimal,
    #[mutable]
    xrd_remaining: Decimal,
    #[mutable]
    fills_to_collect: u64,
    #[mutable]
    fill_percent: Decimal,
    auto_unstake: bool,
}

#[derive(ScryptoSbor, PartialEq, Debug, Clone, Copy)]
pub enum LiquidityStatus {
    Open, 
    Cancelled,
    Closed,
}

#[derive(ScryptoSbor, PartialEq, Debug, Clone, Copy)]
pub enum FillStatus {
    Unfilled,
    Filled,
    PartiallyFilled,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct CombinedKey {
    key: u128,
}

impl CombinedKey {
    pub fn new(liquidity_id: u64, discount_key: u64) -> Self {
        let key = ((liquidity_id as u128) << 64) | (discount_key as u128);
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
#[types(Decimal, ResourceAddress, LiquidityDetails, NonFungibleGlobalId, ComponentAddress, i64, u64, Vault)]
mod liquify_module {
    enable_method_auth! {
        roles {
            owner => updatable_by: [];
        },
        methods {
            add_liquidity => PUBLIC;
            remove_liquidity => PUBLIC;
            liquify_unstake => PUBLIC;
            liquify_unstake_off_ledger => PUBLIC;
            collect_fills => PUBLIC;
            burn_closed_receipts => PUBLIC;
            set_component_status => restrict_to: [owner];
            set_platform_fee => restrict_to: [owner];
            collect_platform_fees => restrict_to: [owner];
            set_max_liquidity_iter => restrict_to: [owner];
            set_max_fills_to_collect => restrict_to: [owner];
            set_minimum_liquidity => restrict_to: [owner];
        }
    }

    struct Liquify {

        liquify_owner_badge: ResourceAddress,
        xrd_liquidity: Vault, // holds all XRD liquidity
        liquidity_receipt: ResourceManager,
        liquidity_receipt_counter: u64,
        max_fills_to_collect: u64, // Maximum number of fills to collect in a single transaction
        buy_list: AvlTree<u128, NonFungibleGlobalId>, // Data structure for all liquidity receipts
        order_fill_tree: AvlTree<u128, UnstakeNFTOrLSU>,  // Data structure for all fills to collect
        component_vaults: KeyValueStore<ResourceAddress, Vault>, // Vaults that store all LSUs and unstake nfts for users
        total_xrd_volume: Decimal,
        total_xrd_locked: Decimal,
        component_status: bool,  // true = active, accepting liquidity false = inactive, not accepting new liquidity
        order_fill_counter: u64,  // Globally increasing counter for order fills
        liquidity_index: Vec<Decimal>,  // Index of total liquidity at each discount level
        discounts: Vec<Decimal>,  // List of discounts available
        platform_fee: Decimal,  // Fee charged to market sellers
        fee_vault: Vault,
        max_liquidity_iter: u64,  // Maximum number of liquidity nfts to iterate through in a single transaction
        minimum_liquidity: Decimal,  // Minimum liquidity required to add a new buy order
    }

    impl Liquify {

        pub fn instantiate_liquify() -> (Global<Liquify>, Bucket) {

            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Liquify::blueprint_id());

            let liquify_owner_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata!(
                    init {
                        "name" => "Liquify Owner Badge".to_string(), locked;
                        "icon_url" => Url::of("https://bafybeicha7fu5nu2j6g7k3siljiqlv6nbu2qbwpcc7jqzzqpios6mrh56i.ipfs.w3s.link/liquify1.jpg"), updatable;
                    }
                ))
                .divisibility(DIVISIBILITY_NONE)
                .mint_initial_supply(1)
                .into();

            let tags = vec!["Liquify", "Liquidity", "LSU"];
            
            let liquidity_receipt = ResourceBuilder::new_integer_non_fungible::<LiquidityDetails>(OwnerRole::Fixed(
                rule!(require_any_of(vec![global_caller(component_address), ResourceOrNonFungible::Resource(liquify_owner_badge.resource_address())]))))
                .metadata(metadata!(
                    init {
                        "name" => "Liquify Liquidity Receipt".to_owned(), updatable;
                        "description" => "Receipt for providing liquidity on the Liquify platform".to_string(), updatable;
                        "icon_url" => Url::of("https://bafybeicha7fu5nu2j6g7k3siljiqlv6nbu2qbwpcc7jqzzqpios6mrh56i.ipfs.w3s.link/liquify1.jpg"), updatable;
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
            
            // Prepare premade bins of liquidity for each allowed discount (5 bps increments from 0 to 5%)
            let mut liquidity_index: Vec<Decimal> = Vec::new();
            let mut discounts: Vec<Decimal> = Vec::new();
            let step: Decimal = dec!(0.00025); // Represents a 0.05% step

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
                max_fills_to_collect: 85,
                buy_list: AvlTree::new(),
                order_fill_tree: AvlTree::new(),
                component_vaults: KeyValueStore::new(),
                liquidity_index,
                discounts,
                total_xrd_volume: Decimal::ZERO,
                total_xrd_locked: Decimal::ZERO,
                component_status: true,
                order_fill_counter: 1,
                platform_fee: dec!(0.00), // 0% fee
                fee_vault: Vault::new(XRD),
                max_liquidity_iter: 28,
                minimum_liquidity: dec!(1000),
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
                    add_liquidity => Xrd(1.into()), updatable;
                    remove_liquidity => Xrd(1.into()), updatable;
                    liquify_unstake => Xrd(1.into()), updatable;
                    liquify_unstake_off_ledger => Xrd(1.into()), updatable;
                    collect_fills => Xrd(1.into()), updatable;
                    burn_closed_receipts => Xrd(1.into()), updatable;
                    set_component_status => Free, updatable;
                    set_platform_fee => Free, updatable;
                    collect_platform_fees => Free, updatable;
                    set_max_liquidity_iter => Free, updatable;
                    set_max_fills_to_collect => Free, updatable;
                    set_minimum_liquidity => Free, updatable;
                }
            })
            .globalize();

            (liquify_component, liquify_owner_badge)
        }

        /// Allows user to deposit XRD liquidity with specified parameters.
        /// 
        /// This method takes a bucket of XRD, a discount of type Decimal that indicates the percentage amount under
        /// the redemption value of an amount of LSUs that a liquidity provider is willing to purchase any LSU, and
        /// a boolean that indicates whether the user wants to automatically unstake any LSUs that are collected.  This
        /// method is contrained by the `minimum_liquidity` variable.  The user must pass in an amount of XRD that is greater
        /// than or equal to the `minimum_liquidity` which is set to 10,000 XRD by default.  This can be adjusted by the owner
        /// of the component in order to maintain an efficient unstaking process.  The higher the minimum, the more liquidity
        /// can be processed in a single transaction.
        /// 
        /// # Arguments
        /// * `xrd_bucket`: A `Bucket` containing XRD to be deposited as liquidity.
        /// * `discount`: A `Decimal` representing the discount percentage the user is willing to use liquidity provided
        /// * `auto_unstake`: A `bool` indicating whether the user wants to automatically unstake any LSUs that are collected.
        ///
        /// # Returns
        /// * A `Bucket` containing the new liquidity receipt NFT that have been minted to track the liquidity.
        pub fn add_liquidity(&mut self, xrd_bucket: Bucket, discount: Decimal, auto_unstake: bool) -> Bucket {
            
            // ensure component is active and user is passing in a large enough amount of XRD
            assert!(self.component_status == true, "Liquify is not accepting new liquidity at this time.");
            assert!(xrd_bucket.resource_address() == XRD, "Bucket must contain XRD");
            assert!(xrd_bucket.amount() >= self.minimum_liquidity, "This amount is below the minimum liquidity requirement XRD");
        
            // Ensure the discount exists
            assert!(self.discounts.contains(&discount), "This discount % is not supported");
        
            // Convert discount to a u64 and combine with order ID into a single u128 key
            let discount_u64 = (discount * dec!(10000)).checked_floor().unwrap().to_string().parse::<u64>().unwrap();

            let combined_key = CombinedKey::new(discount_u64, self.liquidity_receipt_counter);
        
            // Mint new buy order NFT
            let id = NonFungibleLocalId::Integer(IntegerNonFungibleLocalId::new(self.liquidity_receipt_counter));

            let liquidity_receipt_data = LiquidityDetails {
                key_image_url: Url::of("https://bafybeicha7fu5nu2j6g7k3siljiqlv6nbu2qbwpcc7jqzzqpios6mrh56i.ipfs.w3s.link/liquify1.jpg"),
                liquidity_status: LiquidityStatus::Open,
                total_xrd_amount: xrd_bucket.amount(),
                discount: discount,
                xrd_remaining: xrd_bucket.amount(),
                fill_percent: dec!(0),
                fills_to_collect: 0,
                auto_unstake,
            };
        
            let new_liquidity_receipt: Bucket = self.liquidity_receipt.mint_non_fungible(&id, liquidity_receipt_data);
            self.liquidity_receipt_counter += 1;
        
            // Insert the new buy order into the AVL tree
            let global_id = NonFungibleGlobalId::new(self.liquidity_receipt.address(), id);
            self.buy_list.insert(combined_key.key, global_id);
        
            // Update the total liquidity at the correct position in the liquidity index
            let index_usize = match (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>() {
                Ok(index) => index,
                Err(_) => panic!("Failed to calculate liquidity index for discount: {}", discount),
            };

            let currently_liquidity_at_discount = self.liquidity_index[index_usize];
            self.liquidity_index[index_usize] = currently_liquidity_at_discount + xrd_bucket.amount();
        
            // Add to total XRD locked
            self.total_xrd_locked += xrd_bucket.amount();
            
            // Put buy order liquidity in vault
            self.xrd_liquidity.put(xrd_bucket);
        
            // Return the new buy order at the end
            new_liquidity_receipt
        }
        
        /// Allows user to withdraw XRD liquidity using a bucket containing liquidity receipt NFTs.
        /// 
        /// This method takes a bucket of liquidity receipt NFTs and returns the XRD that was deposited in the liquidity pool. This method
        /// is constrained by the `max_fills_to_collect` variable.  Currently this variable is set to 85.  More than 85 fills surpasses the
        /// costing limits of a single transaction.
        /// 
        /// # Arguments
        /// * `liquidity_receipt_bucket`: A `Bucket` containing the liquidity receipt NFTs representing liquidity to remove.
        ///
        /// # Returns
        /// * A `Bucket` containing remaining XRD that was deposited in the liquidity pool.
        pub fn remove_liquidity(&mut self, liquidity_receipt_bucket: Bucket) -> (Bucket, Bucket) {

            // Ensure the bucket contains the buy order NFT
            assert!(liquidity_receipt_bucket.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify buy order NFT(s)");

            // ensure that the liquidity belongs to an order that is open, unfilled, or partially filled
            for local_id in liquidity_receipt_bucket.as_non_fungible().non_fungible_local_ids() {
                let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&local_id);
                assert!(data.liquidity_status == LiquidityStatus::Open, "Order must be Open to remove liquidity");
                assert!(data.xrd_remaining > dec!(0), "Order must be unfilled or partially filled to remove liquidity");
            }

            // Initialize a variable to track the total amount of XRD to return to the user
            let mut total_order_size: Decimal = Decimal::ZERO;
        
            // Iterate over the non-fungible IDs in the liquidity_receipt_bucket
            for local_id in liquidity_receipt_bucket.as_non_fungible().non_fungible_local_ids() {

                // Retrieve liquidity receipt data
                let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&local_id);

                // Ensure the LiquidityStatus is Open
                assert!(data.liquidity_status == LiquidityStatus::Open, "Order must be Open to remove liquidity");

                let discount = data.discount;
                let order_size = data.xrd_remaining;
        
                // Update the liquidity index according to how much XRD is coming out of the liquidity pool
                let index = (discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
                let currently_liquidity_at_discount = self.liquidity_index[index];
                self.liquidity_index[index] = currently_liquidity_at_discount - order_size;
        
                // Add the order size amount to the total order size
                total_order_size += order_size;

                let local_id_u64 = match local_id.clone() {
                    NonFungibleLocalId::Integer(i) => i.value(),
                    _ => 0
                };
        
                // Remove the buy order from the AVL tree using the combined key
                let discount_u64 = (discount * dec!(10000)).checked_floor().unwrap().to_string().parse::<u64>().unwrap();
                let combined_key = CombinedKey::new(discount_u64, local_id_u64).key;

                self.buy_list.remove(&combined_key);
                
                let mut new_order_status: LiquidityStatus = LiquidityStatus::Cancelled;

                self.liquidity_receipt.update_non_fungible_data(&local_id, "xrd_remaining", dec!(0));
                
                if data.fills_to_collect == 0 {
                    new_order_status = LiquidityStatus::Closed;
                }
                
                // Update the buy order data to reflect the cancelled status
                self.liquidity_receipt.update_non_fungible_data(&local_id, "liquidity_status", new_order_status.clone());
            }
        
            // Take the total order size amount from the liquidity vault to return to the user
            let user_funds = self.xrd_liquidity.take(total_order_size);
        
            // Reduce the total XRD locked
            self.total_xrd_locked -= total_order_size;
        
            // Return the XRD taken from the liquidity pool to the user and the bucket of buy order NFTs
            (user_funds, liquidity_receipt_bucket)
        }

        /// Allows users to get an amount of XRD for a given amount of LSUs.
        /// 
        /// Executes the process of iterating through all available liquidity to get the most amount of XRD for a given 
        /// amount of LSUs. It takes all liquidity in order regardless of size of liquidity remaining in the liquidity receipt.
        /// This method is constrained by the `max_liquidity_iter` variable.  When calling the component directly, this method
        /// can handle up to 29 iterations.  If called through the interface component, it can handle up to 28 iterations
        /// 
        /// # Arguments
        /// * `lsu_bucket`: A `Bucket` containing LSUs to be "unstaked" for XRD.
        ///
        /// # Returns
        /// * A `Bucket` containing remaining XRD that was deposited in the liquidity pool.
        /// * A `Bucket` containing any remaining LSUs that were not "unstaked" in case the transaction either hits an iteration
        /// limit or the liquidity pool is empty.
        pub fn liquify_unstake(&mut self, mut lsu_bucket: Bucket) -> (Bucket, Bucket) {

            // Ensure the bucket contains a valid LSU
            assert!(self.validate_lsu(lsu_bucket.resource_address()), "Bucket must contain a native Radix Validator LSU");

            let mut xrd_bucket: Bucket = Bucket::new(XRD); // Initialize an empty bucket to collect XRD
            let mut validator = self.get_validator_from_lsu(lsu_bucket.resource_address()); // Get the validator for the LSU being sold
            let mut redemption_value = validator.get_redemption_value(lsu_bucket.amount());
        
            let mut updates: HashMap<NonFungibleLocalId, (LiquidityDetails, Bucket, Decimal, Decimal)> = HashMap::new();
            let mut lsu_sold_total = Decimal::ZERO; // Track the total amount of LSUs actually sold
            let mut iteration_count = 0; // Track the number of iterations
            
            // Iterate over the buy list, starting from the lowest discount
            self.buy_list.range_mut(0..u128::MAX).for_each(|(avl_key, global_id, _)| {

                if iteration_count >= self.max_liquidity_iter {
                    return scrypto_avltree::IterMutControl::Break;
                }

                let local_id = global_id.local_id();
                let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&local_id);
                let mut xrd_remaining = data.xrd_remaining;
        
                let lsu_amount_to_take;
                let fill_amount;

                let discount = data.discount;
                let discounted_xrd_value_of_lsus = redemption_value * (1 - discount);
        
                // Calculate how much LSU to take and how much XRD to fill
                if discounted_xrd_value_of_lsus <= xrd_remaining {
                    // take remainder of LSUs
                    lsu_amount_to_take = lsu_bucket.amount(); 
                    fill_amount = discounted_xrd_value_of_lsus; 

                    // Update the buy order remaining amount and break the loop as we are done
                    xrd_remaining -= fill_amount;
                    redemption_value = Decimal::ZERO; // To break out of the loop

                } else {

                    // take LSU amount proportional to the remaining XRD in buy order
                    let max_xrd_for_lsu = redemption_value * (1 - discount);
                    lsu_amount_to_take = lsu_bucket.amount() * (xrd_remaining / max_xrd_for_lsu);

                    fill_amount = xrd_remaining;

                    redemption_value = redemption_value * ((lsu_bucket.amount() - lsu_amount_to_take) / lsu_bucket.amount());
                    xrd_remaining = Decimal::ZERO; // No liquidity remaining in this order
                }
        
                let lsu_taken = lsu_bucket.take(lsu_amount_to_take); // Take the calculated amount of LSUs
                let xrd_funds = self.xrd_liquidity.take(fill_amount); // Take the corresponding amount of XRD
                xrd_bucket.put(xrd_funds); // Add the XRD to the xrd_bucket

                lsu_sold_total += lsu_amount_to_take; // Track total LSUs actually sold
        
                updates.insert(local_id.clone(), (data.clone(), lsu_taken, fill_amount, xrd_remaining));


                iteration_count += 1;
    
                if redemption_value.is_zero() {
                    return scrypto_avltree::IterMutControl::Break;
                }
        
                scrypto_avltree::IterMutControl::Continue
            });
        
            // Apply updates after the mutable borrow on `self.buy_list` is done
            for (local_id, (data, lsu_taken, fill_amount, new_remaining)) in updates {
            
                // Update the total liquidity at the correct position in the liquidity index
                let index_usize = (data.discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
                let currently_liquidity_at_discount = self.liquidity_index[index_usize];
                self.liquidity_index[index_usize] = currently_liquidity_at_discount - fill_amount.clone();
        
                // Reconstruct avl_key from local_id and discount, then remove the key if the order is fully filled
                let discount_u64 = (data.discount * dec!(10000)).checked_floor().unwrap().to_string().parse::<u64>().unwrap();
                let local_id_u64 = match local_id.clone() {
                    NonFungibleLocalId::Integer(i) => i.value(),
                    _ => 0,
                };
                
                if new_remaining == dec!(0) {
                    let avl_key = CombinedKey::new(discount_u64, local_id_u64).key;
                    self.buy_list.remove(&avl_key);
                }

                let fill_percent: Decimal = (dec!(1) - (new_remaining / data.total_xrd_amount)) * dec!(100);
                self.liquidity_receipt.update_non_fungible_data(&local_id, "fill_percent", fill_percent);
                
            
                self.liquidity_receipt.update_non_fungible_data(&local_id, "xrd_remaining", new_remaining);

                let mut new_fills_to_collect = data.fills_to_collect;
                new_fills_to_collect += 1;
                self.liquidity_receipt.update_non_fungible_data(&local_id, "fills_to_collect", new_fills_to_collect);
        
                let order_fill_key = CombinedKey::new(local_id_u64, self.order_fill_counter).key;
                self.order_fill_counter += 1;
                
                if data.auto_unstake {
                    let unstake_nft = validator.unstake(lsu_taken);
                    let unstake_nft_data = UnstakeNFTOrLSU::UnstakeNFT(UnstakeNFTData {
                        resource_address: unstake_nft.resource_address(),
                        id: unstake_nft.as_non_fungible().non_fungible_local_id(),
                    });
                    self.order_fill_tree.insert(order_fill_key, unstake_nft_data.clone());
                    self.ensure_user_vault_exists(unstake_nft.resource_address());
                    self.component_vaults.get_mut(&unstake_nft.resource_address()).unwrap().put(unstake_nft);
                } else {
                    let lsu_data = UnstakeNFTOrLSU::LSU(LSUData {
                        resource_address: lsu_taken.resource_address(),
                        amount: lsu_taken.amount(),
                    });
                    self.order_fill_tree.insert(order_fill_key, lsu_data);
                    self.ensure_user_vault_exists(lsu_taken.resource_address());
                    self.component_vaults.get_mut(&lsu_taken.resource_address()).unwrap().put(lsu_taken);
                }
            }

            // Update the total XRD volume after processing
            self.total_xrd_volume += xrd_bucket.amount();
            self.total_xrd_locked -= xrd_bucket.amount();
        
            let fee_bucket =  xrd_bucket.take(xrd_bucket.amount().clone().checked_mul(self.platform_fee).unwrap());
            self.fee_vault.put(fee_bucket);

            // Return the filled XRD bucket and the remaining LSU bucket
            (xrd_bucket, lsu_bucket)
        }

        /// Allows users to get an amount of XRD for a given amount of LSUs.
        /// 
        /// Executes the process of iterating through specified liquidity receipts to "unstake" a bucket of LSUs. 
        /// This method is constrained by the `max_liquidity_iter` variable.  When calling the component directly, this method
        /// can handle up to 29 liquidity receipts.  If called through the interface component, it can handle up to 28 receipts.
        /// 
        /// # Arguments
        /// * `lsu_bucket`: A `Bucket` containing LSUs to be "unstaked" for XRD.
        ///
        /// # Returns
        /// * A `Bucket` containing remaining XRD that was deposited in the liquidity pool.
        /// * A `Bucket` containing any remaining LSUs that were not "unstaked" in case the transaction either hits an iteration
        /// limit or the liquidity pool is empty.
        pub fn liquify_unstake_off_ledger(&mut self, mut lsu_bucket: Bucket, order_keys: Vec<u128>) -> (Bucket, Bucket) {

            // Ensure the bucket contains a valid LSU
            assert!(self.validate_lsu(lsu_bucket.resource_address()), "Bucket must contain a native Radix Validator LSU");

            // Ensure iterations dont encounter costing limits
            assert!(u64::try_from(order_keys.len()).unwrap() <= self.max_liquidity_iter, "Too many receipts to fill in a single transaction");

            let mut xrd_bucket: Bucket = Bucket::new(XRD); // Initialize an empty bucket to collect XRD
            let mut validator = self.get_validator_from_lsu(lsu_bucket.resource_address()); // Get the validator for the LSU being sold
            let mut redemption_value = validator.get_redemption_value(lsu_bucket.amount());
        
            let mut updates = vec![];
            let mut lsu_sold_total = Decimal::ZERO; // Track the total amount of LSUs actually sold

            for key in order_keys {

                let global_id = self.buy_list.get(&key).unwrap();
                let local_id = global_id.local_id();
                let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&local_id);
                let mut xrd_remaining = data.xrd_remaining;
        
                let lsu_amount_to_take;
                let fill_amount;

                let discount = data.discount;
                let discounted_xrd_value_of_lsus: Decimal = redemption_value * (1 - discount);
        
                // Calculate how much LSU to take and how much XRD to fill
                if discounted_xrd_value_of_lsus <= xrd_remaining {
                    
                    // take remainder of LSUs
                    lsu_amount_to_take = lsu_bucket.amount(); 
                    fill_amount = discounted_xrd_value_of_lsus; 

                    // Update the buy order remaining amount and break the loop as we are done
                    xrd_remaining -= fill_amount;
                    redemption_value = Decimal::ZERO; // To break out of the loop

                } else {

                    // take LSU amount proportional to the remaining XRD in buy order
                    let max_xrd_for_lsu = redemption_value * (1 - discount);
                    lsu_amount_to_take = lsu_bucket.amount() * (xrd_remaining / max_xrd_for_lsu);
                    fill_amount = xrd_remaining;

                    
                    redemption_value = redemption_value * ((lsu_bucket.amount() - lsu_amount_to_take) / lsu_bucket.amount());
                    xrd_remaining = Decimal::ZERO; // No liquidity remaining in this order
                }
        
                let lsu_taken = lsu_bucket.take(lsu_amount_to_take); // Take the calculated amount of LSUs
                let xrd_funds = self.xrd_liquidity.take(fill_amount); // Take the corresponding amount of XRD
                xrd_bucket.put(xrd_funds); // Add the XRD to the xrd_bucket

                lsu_sold_total += lsu_amount_to_take; // Track total LSUs actually sold
        
                updates.push((local_id.clone(), data.clone(), lsu_taken, fill_amount, xrd_remaining));
    
                if redemption_value.is_zero() {
                    break;
                }
        
                continue;
            }
        
            // Apply updates after the mutable borrow on `self.buy_list` is done
            for (local_id, data, lsu_taken, fill_amount, new_remaining) in updates {
            
                // Update the total liquidity at the correct position in the liquidity index
                let index_usize = (data.discount / dec!(0.00025)).checked_floor().unwrap().to_string().parse::<usize>().unwrap();
                let currently_liquidity_at_discount = self.liquidity_index[index_usize];
                self.liquidity_index[index_usize] = currently_liquidity_at_discount - fill_amount.clone();
        
                // Reconstruct avl_key from local_id and discount, then remove the key if the order is fully filled
                let discount_u64 = (data.discount * dec!(10000)).checked_floor().unwrap().to_string().parse::<u64>().unwrap();
                let local_id_u64 = match local_id.clone() {
                    NonFungibleLocalId::Integer(i) => i.value(),
                    _ => 0,
                };
                
                if new_remaining == dec!(0) {
                    let avl_key = CombinedKey::new(discount_u64, local_id_u64).key;
                    self.buy_list.remove(&avl_key);
                }

                let fill_percent: Decimal = (dec!(1) - (new_remaining / data.total_xrd_amount)) * dec!(100);
                self.liquidity_receipt.update_non_fungible_data(&local_id, "fill_percent", fill_percent);
                
                self.liquidity_receipt.update_non_fungible_data(&local_id, "xrd_remaining", new_remaining);
                let mut new_fills_to_collect = data.fills_to_collect;
                new_fills_to_collect += 1;
                self.liquidity_receipt.update_non_fungible_data(&local_id, "fills_to_collect", new_fills_to_collect);
        
                let order_fill_key = CombinedKey::new(local_id_u64, self.order_fill_counter).key;
                self.order_fill_counter += 1;
                
                if data.auto_unstake {

                    let unstake_nft = validator.unstake(lsu_taken);
                    let unstake_nft_data = UnstakeNFTOrLSU::UnstakeNFT(UnstakeNFTData {
                        resource_address: unstake_nft.resource_address(),
                        id: unstake_nft.as_non_fungible().non_fungible_local_id(),
                    });
                    self.order_fill_tree.insert(order_fill_key, unstake_nft_data.clone());
                    self.ensure_user_vault_exists(unstake_nft.resource_address());
                    self.component_vaults.get_mut(&unstake_nft.resource_address()).unwrap().put(unstake_nft);

                } else {

                    let lsu_data = UnstakeNFTOrLSU::LSU(LSUData {
                        resource_address: lsu_taken.resource_address(),
                        amount: lsu_taken.amount(),
                    });
                    self.order_fill_tree.insert(order_fill_key, lsu_data);
                    self.ensure_user_vault_exists(lsu_taken.resource_address());
                    self.component_vaults.get_mut(&lsu_taken.resource_address()).unwrap().put(lsu_taken);
                }
            }

            // Update the total XRD volume after processing
            self.total_xrd_volume += xrd_bucket.amount();
            self.total_xrd_locked -= xrd_bucket.amount();
        
            let fee_bucket =  xrd_bucket.take(xrd_bucket.amount().clone().checked_mul(self.platform_fee).unwrap());
            self.fee_vault.put(fee_bucket);
            
            // Return the filled XRD bucket and the remaining LSU bucket
            (xrd_bucket, lsu_bucket)
        }

        /// Allows user to collect fills from liquidity receipt NFTs that have been used in the "unstake" process.
        /// 
        /// This method takes a bucket of liquidity receipt NFTs and returns either LSUs or stake claim NFTs depending
        /// on whether or not the auto unstake feature was selected for the liquidity receipt. This method
        /// is constrained by the `max_fills_to_collect` variable.  Currently this variable is set to 85.  More than 85 fills surpasses the
        /// costing limits of a single transaction.
        /// 
        /// # Arguments
        /// * `liquidity_receipt_bucket`: A `Bucket` containing the liquidity receipt NFTs representing liquidity to remove.
        ///
        /// # Returns
        /// * A `Vec<Bucket>` containing LSUs or stake claim NFTs from the unstaking process.
        /// * A `Bucket` containing the original liquidity receipt NFTs with updated data.
        pub fn collect_fills(&mut self, liquidity_receipt_bucket: Bucket) -> (Vec<Bucket>, Bucket) {
            
            // Ensure bucket contains a real liquidity receipt
            assert!(
                liquidity_receipt_bucket.resource_address() == self.liquidity_receipt.address(),
                "Bucket must contain Liquify liquidity receipts NFT(s)"
            );
        
            let mut updates = vec![];  // Collecting updates to apply after processing
            let mut bucket_vec: Vec<Bucket> = Vec::new();  // Collected buckets for returning unstake NFTs or LSUs
            let mut collect_counter: u64 = 0;  // Total number of fills collected
        
            // Iterate over the non-fungible IDs in the liquidity_receipt_bucket (loop over the NFTs)
            for order_id in liquidity_receipt_bucket.as_non_fungible().non_fungible_local_ids() {
                
                // Retrieve order data
                let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&order_id);
        
                // Skip this order if no fills are available to collect
                if data.fills_to_collect == 0 || collect_counter >= self.max_fills_to_collect {
                    continue;
                }
        
                // Convert order ID to u64
                let order_id_u64 = match order_id.clone() {
                    NonFungibleLocalId::Integer(i) => i.value(),
                    _ => 0,
                };
        
                // Calculate the start and end keys directly based on the order ID
                let start_key = CombinedKey::new(order_id_u64, 1).key;
                let end_key = CombinedKey::new(order_id_u64, u64::MAX).key;
        
                // Loop over the AVL tree to collect fills for this order
                let mut fills_collected_for_this_order: u64 = 0;
        
                self.order_fill_tree.range_mut(start_key..=end_key).for_each(
                    |(avl_key, unstake_nft_or_lsu, _next_key): (&u128, &mut UnstakeNFTOrLSU, Option<u128>)| {
                        // Break if we've collected the maximum allowed number of fills
                        if collect_counter >= self.max_fills_to_collect {
                            return scrypto_avltree::IterMutControl::Break;
                        }
        
                        // Does this order fill represent an unstake NFT or an LSU?
                        match unstake_nft_or_lsu {
                            // If this fill is an LSU, collect it and add to the bucket vector
                            UnstakeNFTOrLSU::LSU(lsu_data) => {
                                let mut lsu_bucket = Bucket::new(lsu_data.resource_address);
                                let lsu_resource = lsu_data.resource_address;
                                let lsu_amount = lsu_data.amount;
                                let mut lsu_vault = self.component_vaults.get_mut(&lsu_resource).unwrap();
                                lsu_bucket.put(lsu_vault.take(lsu_amount));
                                bucket_vec.push(lsu_bucket);
                            }
        
                            // If this fill is an unstake NFT, collect it and add to the bucket vector
                            UnstakeNFTOrLSU::UnstakeNFT(unstake_nft_data) => {
                                let mut unstake_nft_bucket = Bucket::new(unstake_nft_data.resource_address);
                                let unstake_nft_id = &unstake_nft_data.id;
                                let unstake_nft_vault = self.component_vaults.get_mut(&unstake_nft_data.resource_address).unwrap();
                                unstake_nft_bucket.put(unstake_nft_vault.as_non_fungible().take_non_fungible(&unstake_nft_id).into());
                                bucket_vec.push(unstake_nft_bucket);
                            }
                        }
        
                        // Mark this fill for removal from the AVL tree and update the collect count
                        updates.push((*avl_key, order_id.clone(), data.fills_to_collect - 1));
                        fills_collected_for_this_order += 1;
                        collect_counter += 1;
        
                        scrypto_avltree::IterMutControl::Continue
                    },
                );
        
                // Update the order with how many fills were collected for this specific NFT
                let new_fills_to_collect = data.fills_to_collect - fills_collected_for_this_order;
                updates.push((start_key, order_id.clone(), new_fills_to_collect));
            }
        
            // Remove all collected fills from the AVL tree and update the buy order data
            for (avl_key_to_remove, order_id, new_fills_to_collect) in updates {
                self.order_fill_tree.remove(&avl_key_to_remove);
                self.liquidity_receipt.update_non_fungible_data(&order_id, "fills_to_collect", new_fills_to_collect);
        
                // Update the order status if needed (if all fills collected and no remaining amount)
                let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&order_id);
                if data.xrd_remaining == dec!(0) && data.fills_to_collect == 0 {
                    self.liquidity_receipt.update_non_fungible_data(&order_id, "liquidity_status", LiquidityStatus::Closed);
                }
            }
        
            // Return the collected fills and the original buy order bucket
            (bucket_vec, liquidity_receipt_bucket)
        }

        /// Allows user an option to burn closed liquidity receipts if they wish.
        /// 
        /// This method checks that liquidity receipts input are closed and then burns them.
        /// 
        /// # Arguments
        /// * `receipts`: A `Bucket` containing the liquidity receipt NFTs.
        ///
        /// # Returns
        /// * None
        pub fn burn_closed_receipts(&mut self, receipts: Bucket) {

            // Ensure the bucket contains a liquidity receipt
            assert!(receipts.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify liquidity receipt(s)");

            // Iterate over the non-fungible IDs in the liquidity_receipt_bucket
            for local_id in receipts.as_non_fungible().non_fungible_local_ids() {

                // Retrieve receipt data
                let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&local_id);

                // Ensure the LiquidityStatus is Closed
                assert!(data.liquidity_status == LiquidityStatus::Closed, "Liquidity receipt status must be Closed to burn");
            }

            receipts.burn();
        }
        
        /// Allows protocol owner to collect any fees that have been generated by the platform.
        /// 
        /// # Requires
        /// * Proof of owner badge
        /// 
        /// # Arguments
        /// * None
        ///
        /// # Returns
        /// * A `Bucket` containing all available XRD from component vault.
        pub fn collect_platform_fees(&mut self) -> Bucket {
            self.fee_vault.take_all()
        }
        
        /// Allows protocol owner to collect any fees that have been generated by the platform.
        /// 
        /// # Requires
        /// * Proof of owner badge
        /// 
        /// # Arguments
        /// * 'bool' - A boolean value to set the status of the component: true for active, false for inactive.
        ///
        /// # Returns
        /// * None
        pub fn set_component_status(&mut self, status: bool) {
            self.component_status = status;
        }

        /// Allows protocol owner to collect any fees that have been generated by the platform.
        /// 
        /// # Requires
        /// * Proof of owner badge
        /// 
        /// # Arguments
        /// * 'bool' - A boolean value to set the status of the component: true for active, false for inactive.
        ///
        /// # Returns
        /// * None
        pub fn set_platform_fee(&mut self, fee: Decimal) {
            self.platform_fee = fee;
        }

        /// Allows protocol owner to set the maximum number of liquidity receipts that can be processed in a single transaction.
        /// Current logic allows for up to 29 receipts to be processed in a single transaction directly or 28 through the interface.
        /// 
        /// # Requires
        /// * Proof of owner badge
        /// 
        /// # Arguments
        /// * 'u64' - An integer number of receipts to process in a single transaction.
        ///
        /// # Returns
        /// * None
        pub fn set_max_liquidity_iter(&mut self, max: u64) {
            self.max_liquidity_iter = max;
        }

        /// Allows protocol owner to set the maximum number of fills that can be processed in a single transaction.
        /// Current logic allows for up to 85 fills to be collected in a single transaction.
        /// 
        /// # Requires
        /// * Proof of owner badge
        /// 
        /// # Arguments
        /// * 'u64' - An integer number of fills to collect in a single transaction.
        ///
        /// # Returns
        /// * None
        pub fn set_max_fills_to_collect(&mut self, max: u64) {
            self.max_fills_to_collect = max;
        }

        /// Allows protocol owner to set the minimum deposit amount for liquidity.  The higher the minimum, the larger
        /// the amount of liquidity is that can be processed in a single transaction.
        /// 
        /// # Requires
        /// * Proof of owner badge
        /// 
        /// # Arguments
        /// * 'Decimal' - mimimum liquidity of XRD for depositing into the liquidity pool.
        ///
        /// # Returns
        /// * None
        pub fn set_minimum_liquidity(&mut self, min: Decimal) {
            self.minimum_liquidity = min;
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
