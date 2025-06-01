use scrypto_test::prelude::*;

#[derive(Clone)]
pub struct Account {
    public_key: Secp256k1PublicKey,  // <-- This was the issue: was "pub_key"
    account_address: ComponentAddress,
} 

pub struct TestEnvironment {
    pub ledger: LedgerSimulator<NoExtension, InMemorySubstateDatabase>,
    pub admin_account: Account,
    pub user_account1: Account,
    pub user_account2: Account,
    pub user_account3: Account,
    pub user_account4: Account,
    pub user_account5: Account,
    pub user_account6: Account,
    pub package_address: PackageAddress,
    pub liquify_component: ComponentAddress,
    pub liquify_interface_component: ComponentAddress,
    pub owner_badge: ResourceAddress,
    pub liquidity_receipt: ResourceAddress,
    pub lsu_resource_address: ResourceAddress,
}

impl TestEnvironment {
    pub fn instantiate_test() -> Self {
        let mut ledger = LedgerSimulatorBuilder::new().without_kernel_trace().build();

        // Create accounts
        let (admin_public_key, _admin_private_key, admin_account_address) = ledger.new_allocated_account();
        let admin_account = Account { public_key: admin_public_key, account_address: admin_account_address };

        let (user_public_key1, _user_private_key1, user_account_address1) = ledger.new_allocated_account();
        let user_account1 = Account { public_key: user_public_key1, account_address: user_account_address1 };

        let (user_public_key2, _user_private_key2, user_account_address2) = ledger.new_allocated_account();
        let  user_account2 = Account { public_key: user_public_key2, account_address: user_account_address2 };

        let (user_public_key3, _user_private_key3, user_account_address3) = ledger.new_allocated_account();
        let  user_account3 = Account { public_key: user_public_key3, account_address: user_account_address3 };

        let (user_public_key4, _user_private_key4, user_account_address4) = ledger.new_allocated_account();
        let  user_account4 = Account { public_key: user_public_key4, account_address: user_account_address4 };

        let (user_public_key5, _user_private_key5, user_account_address5) = ledger.new_allocated_account();
        let  user_account5 = Account { public_key: user_public_key5, account_address: user_account_address5 };

        let (user_public_key6, _user_private_key6, user_account_address6) = ledger.new_allocated_account();
        let  user_account6 = Account { public_key: user_public_key6, account_address: user_account_address6 };

        let package_address = ledger.compile_and_publish(this_package!());

        // *********** Instantiate Interface component ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .call_function(
                package_address,
                "LiquifyInterface",
                "instantiate_interface",
                manifest_args!(),
            )
            .call_method(
                admin_account_address,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&admin_public_key)],
        );
        println!("{:?}\n", receipt);

        let interface_component = receipt.expect_commit(true).new_component_addresses()[0];

        // *********** Instantiate Liquify component ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .call_function(
                package_address,
                "Liquify",
                "instantiate_liquify",
                manifest_args!(),
            )
            .call_method(
                admin_account_address,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&admin_public_key)],
        );
        println!("{:?}\n", receipt);

        let liquify_component = receipt.expect_commit(true).new_component_addresses()[0];
        let owner_badge = receipt.expect_commit(true).new_resource_addresses()[0];
        let liquidity_receipt = receipt.expect_commit(true).new_resource_addresses()[1];

        // *********** Set interface target ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .call_method(
                interface_component, 
                "set_interface_target", 
                manifest_args!(liquify_component),
            )
            .call_method(
                admin_account_address,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&admin_public_key)],
        );
        println!("{:?}\n", receipt);
        receipt.expect_commit_success();

        // *********** Setup LSUs ***********
        let key = Secp256k1PrivateKey::from_u64(1u64).unwrap().public_key();
        let validator_address = ledger.get_active_validator_with_key(&key);
        let lsu_resource_address = ledger
            .get_active_validator_info_by_key(&key)
            .stake_unit_resource;

        // Give users LSUs
        for (user_account, user_public_key) in [
            (user_account_address1, user_public_key1),
            (user_account_address2, user_public_key2),
            (user_account_address3, user_public_key3),
        ] {
            let manifest = ManifestBuilder::new()
                .lock_fee_from_faucet() 
                .withdraw_from_account(user_account, XRD, dec!(1000))
                .take_all_from_worktop(XRD, "xrd")
                .call_method_with_name_lookup(validator_address, "stake", |lookup| {
                    (lookup.bucket("xrd"),)
                })
                .call_method(
                    user_account,
                    "deposit_batch",
                    manifest_args!(ManifestExpression::EntireWorktop),
                )
                .build();

            let receipt = ledger.execute_manifest(
                manifest,
                vec![NonFungibleGlobalId::from_public_key(&user_public_key)],
            );
            receipt.expect_commit_success();
        }

        // *********** Set minimum liquidity to 0 ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .create_proof_from_account_of_amount(
                admin_account_address, 
                owner_badge,
                1,
            )
            .call_method(
                liquify_component, 
                "set_minimum_liquidity", 
                manifest_args!(dec!("0")),
            )
            .call_method(
                admin_account_address,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&admin_public_key)],
        );
        receipt.expect_commit_success();

        // *********** Add liquidity through interface ***********
        for _ in 0..30 {
            for (discount, auto_unstake, auto_refill) in [
                (dec!("0.010"), true, false),
                (dec!("0.020"), true, false),
                (dec!("0.025"), true, true),
            ] {
                let refill_threshold = if auto_refill { dec!("10000") } else { dec!("0") };
                
                let manifest = ManifestBuilder::new()
                    .lock_fee_from_faucet()
                    .withdraw_from_account(user_account_address4, XRD, dec!(10))
                    .take_all_from_worktop(XRD, "xrd")
                    .call_method_with_name_lookup(interface_component, "add_liquidity", |lookup| {(
                        lookup.bucket("xrd"),
                        discount,
                        auto_unstake,
                        auto_refill,
                        refill_threshold,
                    )})
                    .call_method(
                        user_account_address4,
                        "deposit_batch",
                        manifest_args!(ManifestExpression::EntireWorktop),
                    )
                    .build();
                let receipt = ledger.execute_manifest(
                    manifest,
                    vec![NonFungibleGlobalId::from_public_key(&user_public_key4)],
                );
                receipt.expect_commit_success();
            }
        }
        
        Self {
            ledger,
            admin_account,
            user_account1,
            user_account2,
            user_account3,
            user_account4,
            user_account5,
            user_account6,
            package_address,
            liquify_component,
            liquify_interface_component: interface_component,
            owner_badge,
            liquidity_receipt,
            lsu_resource_address,
        }
    }

    pub fn execute_manifest(
        &mut self,
        manifest: TransactionManifestV1, 
        account: Account,
    ) -> TransactionReceipt {
        self.ledger.execute_manifest(
            manifest, 
            vec![NonFungibleGlobalId::from_public_key(&account.public_key)]
        )
    }
}

















#[test]
fn instantiate_test() {
    TestEnvironment::instantiate_test();
}












#[test]
fn test_interface_iterations() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let liquify_interface_component = ledger.liquify_interface_component;
    let lsu_resource_address = ledger.lsu_resource_address;

    // Test unstaking through interface with iteration limit
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(1000)
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_interface_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            31u8  // max_iterations for interface
        )
        })
        .call_method(
            user_account1,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
        
    let receipt = ledger.execute_manifest(
        manifest,
        ledger.user_account1.clone(),
    );
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}













#[test]
fn test_interface_automation() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account4 = ledger.user_account4.account_address;
    let liquify_interface_component = ledger.liquify_interface_component;
    let liquidity_receipt = ledger.liquidity_receipt;

    // Add liquidity with automation enabled through interface
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(user_account4, XRD, dec!(20000))
        .take_all_from_worktop(XRD, "xrd")
        .call_method_with_name_lookup(liquify_interface_component, "add_liquidity", |lookup| {(
            lookup.bucket("xrd"),
            dec!("0.01"),   // 1% discount
            true,           // auto_unstake
            true,           // auto_refill
            dec!("10000"),  // refill_threshold
        )})
        .call_method(
            user_account4,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = ledger.execute_manifest(
        manifest,
        ledger.user_account4.clone(),
    );
    
    // Just verify the transaction succeeded
    receipt.expect_commit_success();

    // Test update automation through interface
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account4,
            liquidity_receipt,
            dec!(1)
        )
        .take_all_from_worktop(liquidity_receipt, "receipt_bucket")
        .call_method_with_name_lookup(liquify_interface_component, "update_automation", |lookup| {
            (lookup.bucket("receipt_bucket"),
            false,         // Turn off auto_refill
            dec!("0"),     // refill_threshold
        )
        })
        .call_method(
            user_account4,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
        
    let receipt = ledger.execute_manifest(
        manifest,
        ledger.user_account4.clone(),
    );
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}















#[test]
fn test_interface_increase_liquidity() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account5 = ledger.user_account5.account_address;
    let liquify_interface_component = ledger.liquify_interface_component;
    let liquidity_receipt = ledger.liquidity_receipt;

    // First add some liquidity
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(user_account5, XRD, dec!(1000))
        .take_all_from_worktop(XRD, "xrd")
        .call_method_with_name_lookup(liquify_interface_component, "add_liquidity", |lookup| {(
            lookup.bucket("xrd"),
            dec!("0.02"),   // 2% discount
            false,          // auto_unstake
            false,          // auto_refill
            dec!("0"),      // refill_threshold
        )})
        .call_method(
            user_account5,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = ledger.execute_manifest(
        manifest,
        ledger.user_account5.clone(),
    );
    receipt.expect_commit_success();

    // Then increase liquidity through interface
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account5,
            liquidity_receipt,
            dec!(1)
        )
        .withdraw_from_account(user_account5, XRD, dec!(500))
        .take_all_from_worktop(liquidity_receipt, "receipt_bucket")
        .take_all_from_worktop(XRD, "xrd_bucket")
        .call_method_with_name_lookup(liquify_interface_component, "increase_liquidity", |lookup| {
            (lookup.bucket("receipt_bucket"),
            lookup.bucket("xrd_bucket"),
        )
        })
        .call_method(
            user_account5,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
        
    let receipt = ledger.execute_manifest(
        manifest,
        ledger.user_account5.clone(),
    );
    println!("{:?}\n", receipt);
    receipt.expect_commit_success();
}