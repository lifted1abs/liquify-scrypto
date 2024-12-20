use scrypto::prelude::*;
// mod crate::liquify_module;
use crate::liquify::liquify_module::Liquify;

#[blueprint]
#[types(ComponentAddress, ResourceAddress, u32)]
mod interface_module {

    // const BOBBY_MASTER_KEY: ResourceManager = resource_manager!(
    //     "resource_rdx1tkef2p3ldpxv2zqydltw64730yt2stjgxq7xs08lqw7x8yfl3avncz"
    // );

    // const BOBBY_MASTER_KEY: ResourceManager = resource_manager!(
    //     "resource_tdx_2_1t5yldg4y2vwe5ydpvr53wryydc8afccyq429htg9q7awj44pn0stlu"
    // );


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
            set_interface_target => PUBLIC;

            // burn_closed_orders => PUBLIC;
            // set_component_status => restrict_to: [owner];
            // set_platform_fee => restrict_to: [owner];
            // collect_platform_fees => restrict_to: [owner];
            // set_max_liquidity_iter => restrict_to: [owner];
            // set_max_fills_to_collect => restrict_to: [owner];
            // set_minimum_liquidity => restrict_to: [owner];
        }
    }

    struct LiquifyInterface {
        liquify_interface_owner_badge: ResourceAddress,
        active_liquify_component_address: Option<ComponentAddress>,
        active_liquify_component: Option<Global<Liquify>>,
    }

    impl LiquifyInterface {
        pub fn instantiate_interface() -> (Bucket, Global<LiquifyInterface>) {

            let (address_reservation, component_address) =
                Runtime::allocate_component_address(<LiquifyInterface>::blueprint_id());

            // let lootbox_manager_component_address = component_address.clone();

            // let lootbox_manager_virtual_badge = NonFungibleGlobalId::global_caller_badge(component_address);

            let liquify_interface_owner_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata!(
                    init {
                        "name" => "Liquify Owner Badge".to_string(), locked;
                        "icon_url" => Url::of("https://bafybeicha7fu5nu2j6g7k3siljiqlv6nbu2qbwpcc7jqzzqpios6mrh56i.ipfs.w3s.link/liquify1.jpg"), updatable;
                    }
                ))
                .divisibility(DIVISIBILITY_NONE)
                .mint_initial_supply(1)
                .into();
                
            let interface_component = LiquifyInterface {
                active_liquify_component_address: None,
                active_liquify_component: None,
                liquify_interface_owner_badge: liquify_interface_owner_badge.resource_address()
            }

            .instantiate()
            .prepare_to_globalize(
                OwnerRole::Fixed(
                    rule!(require(liquify_interface_owner_badge.resource_address())
                )
            ))
            .roles(roles!(
                owner => rule!(require(liquify_interface_owner_badge.resource_address()));
            ))
            .with_address(address_reservation)
            .metadata(metadata!(
                init {
                    "name" => "Bobby Lootbox Manager".to_string(), updatable;
                }
            ))
            .globalize();

            (liquify_interface_owner_badge, interface_component)
        } 

        pub fn set_interface_target(&mut self, component_address: ComponentAddress) {
            self.active_liquify_component_address = Some(component_address);
            self.active_liquify_component = Some(component_address.into());
        }
        
        pub fn add_liquidity(&mut self, xrd_bucket: Bucket, discount: Decimal, auto_unstake: bool) -> Bucket {
            
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();

            let receipt_bucket = liquify_component.add_liquidity(xrd_bucket, discount, auto_unstake);

            receipt_bucket
        }
        
        pub fn remove_liquidity(&mut self, liquidity_receipt_bucket: Bucket) -> (Bucket, Bucket) {

            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (liquidity_bucket, liquidity_receipt_bucket) = liquify_component.remove_liquidity(liquidity_receipt_bucket);

            (liquidity_bucket, liquidity_receipt_bucket)
        }

        pub fn liquify_unstake(&mut self, lsu_bucket: Bucket) -> (Bucket, Bucket) {

            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (xrd_bucket, lsu_bucket) = liquify_component.liquify_unstake(lsu_bucket);

            (xrd_bucket, lsu_bucket)
        }

        // User can pass in a specific selection of orders/keys from the AvlTree to fill directly
        pub fn liquify_unstake_off_ledger(&mut self, lsu_bucket: Bucket, order_keys: Vec<u128>) -> (Bucket, Bucket) {

            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (xrd_bucket, lsu_bucket) = liquify_component.liquify_unstake_off_ledger(lsu_bucket, order_keys);

            (xrd_bucket, lsu_bucket)
        }

        pub fn collect_fills(&mut self, liquidity_receipt_bucket: Bucket) -> (Vec<Bucket>, Bucket) {
            
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (bucket_vec, liquidity_receipt_bucket) = liquify_component.collect_fills(liquidity_receipt_bucket);
        
            // Return the collected fills and the original buy order bucket
            (bucket_vec, liquidity_receipt_bucket)
        }
        
        // pub fn collect_platform_fees(&mut self) -> Bucket {
        //     self.fee_vault.take_all()
        // }
        
        // pub fn set_component_status(&mut self, status: Decimal) {
        //     self.component_status = status;
        //     // info!("Component status set to: {}", status);
        // }

        // pub fn set_platform_fee(&mut self, fee: Decimal) {
        //     self.platform_fee = fee;
        // }

        // pub fn set_max_liquidity_iter(&mut self, max: u64) {
        //     self.max_liquidity_iter = max;
        // }

        // pub fn set_max_fills_to_collect(&mut self, max: u64) {
        //     self.max_fills_to_collect = max;
        // }

        // pub fn set_minimum_liquidity(&mut self, min: Decimal) {
        //     self.minimum_liquidity = min;
        // }

        // pub fn burn_closed_orders(&mut self, orders: Bucket) {
        //     // Ensure the bucket contains the buy order NFT
        //     assert!(orders.resource_address() == self.liquidity_receipt.address(), "Bucket must contain Liquify buy order NFT(s)");

        //     // Iterate over the non-fungible IDs in the liquidity_receipt_bucket
        //     for local_id in orders.as_non_fungible().non_fungible_local_ids() {
        //         // Retrieve buy order data
        //         let data: LiquidityDetails = self.liquidity_receipt.get_non_fungible_data(&local_id);

        //         // Ensure the LiquidityStatus is Closed
        //         assert!(data.liquidity_status == LiquidityStatus::Closed, "Order must be Closed to burn");
        //     }

        //     orders.burn();
        // } 


    }
}