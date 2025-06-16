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
            increase_liquidity => PUBLIC;
            remove_liquidity => PUBLIC;
            liquify_unstake => PUBLIC;
            liquify_unstake_off_ledger => PUBLIC;
            collect_fills => PUBLIC;
            update_automation => PUBLIC;
            cycle_liquidity => PUBLIC;
            get_claimable_xrd => PUBLIC;
            get_liquidity_data => PUBLIC;
            set_interface_target => PUBLIC;
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

            let liquify_interface_owner_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .metadata(metadata!(
                    init {
                        "name" => "Liquify Interface Owner Badge".to_string(), locked;
                        "icon_url" => Url::of("https://bafybeif5tjpcgjgfo2lt6pp3qnz5s7mdpejfhgkracs7hzreoeg3bw3wae.ipfs.w3s.link/liquify_icon.png"), updatable;
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
        
        pub fn add_liquidity(
            &mut self, 
            xrd_bucket: Bucket, 
            discount: Decimal, 
            auto_unstake: bool,
            auto_refill: bool,
            refill_threshold: Decimal
        ) -> NonFungibleBucket {
            
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();

            let receipt_bucket = liquify_component.add_liquidity(
                xrd_bucket, 
                discount, 
                auto_unstake,
                auto_refill,
                refill_threshold
            );

            receipt_bucket
        }

        pub fn increase_liquidity(&mut self, receipt_bucket: Bucket, xrd_bucket: Bucket) -> Bucket {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.increase_liquidity(receipt_bucket, xrd_bucket)
        }
        
        pub fn remove_liquidity(&mut self, liquidity_receipt_bucket: Bucket) -> (Bucket, Bucket) {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (liquidity_bucket, liquidity_receipt_bucket) = liquify_component.remove_liquidity(liquidity_receipt_bucket);

            (liquidity_bucket, liquidity_receipt_bucket)
        }

        pub fn liquify_unstake(&mut self, lsu_bucket: FungibleBucket, max_iterations: u8) -> (Bucket, FungibleBucket) {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
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

        pub fn update_automation(
            &mut self, 
            receipt_bucket: Bucket, 
            auto_refill: bool, 
            refill_threshold: Decimal
        ) -> Bucket {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.update_automation(receipt_bucket, auto_refill, refill_threshold)
        }

        pub fn cycle_liquidity(&mut self, receipt_id: NonFungibleLocalId, max_fills_to_process: u64) -> FungibleBucket {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.cycle_liquidity(receipt_id.into(), max_fills_to_process)
        }
        
        pub fn get_claimable_xrd(&self, receipt_id: NonFungibleLocalId) -> Decimal {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.get_claimable_xrd(receipt_id)
        }

        pub fn get_liquidity_data(&self, receipt_id: NonFungibleLocalId) -> crate::liquify::LiquidityData {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.get_liquidity_data(receipt_id)
        }
    }
}