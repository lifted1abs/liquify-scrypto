// src/interface.rs

use scrypto::prelude::*;
use crate::liquify::{liquify_module::Liquify, LiquidityData, ReceiptDetailData, AutomationReadyReceipt};


#[blueprint]
#[types(ComponentAddress, ResourceAddress, u32, LiquidityData, ReceiptDetailData, AutomationReadyReceipt)]
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
            update_auto_refill_status => PUBLIC;
            update_refill_threshold => PUBLIC;
            cycle_liquidity => PUBLIC;
            get_claimable_xrd => PUBLIC;
            get_raw_buy_list_range => PUBLIC;
            get_automation_ready_receipts => PUBLIC;
            get_receipt_detail => PUBLIC;
            get_active_liquidity_positions => PUBLIC;
            set_interface_target => restrict_to: [owner];
        }
    }

    struct LiquifyInterface {
        liquify_interface_owner_badge: ResourceAddress,
        active_liquify_component_address: Option<ComponentAddress>,
        active_liquify_component: Option<Global<Liquify>>,
    }

    impl LiquifyInterface {
        pub fn instantiate_interface() -> (Global<LiquifyInterface>, Bucket) {

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

            (interface_component, liquify_interface_owner_badge)
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
            refill_threshold: Decimal,
            automation_fee: Decimal 
        ) -> NonFungibleBucket {
            
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();

            let receipt_bucket = liquify_component.add_liquidity(
                xrd_bucket, 
                discount, 
                auto_unstake,
                auto_refill,
                refill_threshold,
                automation_fee  
            );

            receipt_bucket
        }

        pub fn increase_liquidity(&mut self, receipt_bucket: Bucket, xrd_bucket: Bucket) -> Bucket {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.increase_liquidity(receipt_bucket, xrd_bucket)
        }

        pub fn remove_liquidity(&mut self, liquidity_receipt_bucket: Bucket) -> (Bucket, Bucket) {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.remove_liquidity(liquidity_receipt_bucket)
        }

        pub fn liquify_unstake(&mut self, lsu_bucket: Bucket, max_iterations: u8) -> (Bucket, Bucket) {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (xrd_bucket, remaining_lsu) = liquify_component.liquify_unstake(lsu_bucket.as_fungible(), max_iterations);
            (xrd_bucket, remaining_lsu.into())
        }

        pub fn liquify_unstake_off_ledger(&mut self, lsu_bucket: Bucket, order_keys: Vec<u128>) -> (Bucket, Bucket) {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            let (xrd_bucket, remaining_lsu) = liquify_component.liquify_unstake_off_ledger(lsu_bucket.as_fungible(), order_keys);
            
            (xrd_bucket, remaining_lsu.into())
        }

        pub fn collect_fills(&mut self, receipt_bucket: Bucket, number_of_fills_to_collect: u64) -> (Vec<Bucket>, Bucket) {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.collect_fills(receipt_bucket, number_of_fills_to_collect)
        }

        pub fn update_auto_refill_status(&mut self, receipt_bucket: Bucket, auto_refill: bool) -> Bucket {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.update_auto_refill_status(receipt_bucket, auto_refill)
        }

        pub fn update_refill_threshold(&mut self, receipt_bucket: Bucket, refill_threshold: Decimal) -> Bucket {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.update_refill_threshold(receipt_bucket, refill_threshold)
        }

        pub fn cycle_liquidity(&mut self, receipt_ids: Vec<NonFungibleLocalId>) -> Bucket {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.cycle_liquidity(receipt_ids).into()
        }

        pub fn get_claimable_xrd(&self, receipt_id: NonFungibleLocalId) -> (Decimal, u64, Decimal, Decimal) {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.get_claimable_xrd(receipt_id)
        }

        pub fn get_receipt_detail(&self, receipt_id: NonFungibleLocalId) -> ReceiptDetailData {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.get_receipt_detail(receipt_id)
        }

        pub fn get_automation_ready_receipts(&self, start_index: u64, batch_size: u64) -> Vec<AutomationReadyReceipt> {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.get_automation_ready_receipts(start_index, batch_size)
        }

        pub fn get_raw_buy_list_range(&self, start_index: u64, count: u64) -> Vec<(u128, NonFungibleGlobalId)> {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.get_raw_buy_list_range(start_index, count)
        }

        pub fn get_active_liquidity_positions(&self, start_index: u64, count: u64) -> Vec<ReceiptDetailData> {
            let liquify_component: Global<Liquify> = self.active_liquify_component.unwrap().into();
            
            liquify_component.get_active_liquidity_positions(start_index, count)
        }
    }
}