use scrypto::prelude::*;
use crate::liquify::liquify_module::Liquify;

#[blueprint]
#[types(ComponentAddress, ResourceAddress, u32)]
mod interface_module {

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
            burn_closed_receipts => PUBLIC;
        }
    }

    struct LiquifyInterface {

        liquify_interface_owner_badge: ResourceAddress,
        active_liquify_component_address: Option<ComponentAddress>,
        active_liquify_component: Option<Global<Liquify>>,
    }

    impl LiquifyInterface {
        pub fn instantiate_interface() -> (Bucket, Global<LiquifyInterface>) {

            let (address_reservation, _component_address) =
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
                    "name" => "Liquify User Interface".to_string(), updatable;
                }
            ))
            .globalize();

            (liquify_interface_owner_badge, interface_component)
        } 

        pub fn set_interface_target(&mut self, component_address: ComponentAddress) {

            self.active_liquify_component_address = Some(component_address);

            self.active_liquify_component = Some(component_address.into());
        }
        
        pub fn add_liquidity(&mut self, xrd_bucket: Bucket, discount: Decimal, auto_unstake: bool) -> NonFungibleBucket {
            
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();

            let receipt_bucket = liquify_component.add_liquidity(xrd_bucket, discount, auto_unstake);

            receipt_bucket
        }
        
        pub fn remove_liquidity(&mut self, liquidity_receipt_bucket: Bucket) -> (Bucket, Bucket) {

            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (liquidity_bucket, liquidity_receipt_bucket) = liquify_component.remove_liquidity(liquidity_receipt_bucket);

            (liquidity_bucket, liquidity_receipt_bucket)
        }

        pub fn liquify_unstake(&mut self, lsu_bucket: FungibleBucket, max_iterations: u8) -> (Bucket, FungibleBucket) {

            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            // Pass through the user-provided max_iterations
            let (xrd_bucket, lsu_bucket) = liquify_component.liquify_unstake(lsu_bucket, max_iterations);
        
            (xrd_bucket, lsu_bucket)
        }

        pub fn liquify_unstake_off_ledger(&mut self, lsu_bucket: FungibleBucket, order_keys: Vec<u128>) -> (Bucket, FungibleBucket) {

            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (xrd_bucket, lsu_bucket) = liquify_component.liquify_unstake_off_ledger(lsu_bucket, order_keys);

            (xrd_bucket, lsu_bucket)
        }

        pub fn collect_fills(&mut self, liquidity_receipt_bucket: Bucket, number_of_fills_to_collect: u64) -> (Vec<Bucket>, Bucket) {
            
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (bucket_vec, liquidity_receipt_bucket) = liquify_component.collect_fills(liquidity_receipt_bucket, number_of_fills_to_collect);
        
            (bucket_vec, liquidity_receipt_bucket)
        }

        pub fn burn_closed_receipts(&mut self, orders: Bucket) {

            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.burn_closed_receipts(orders);
        } 

    }
}