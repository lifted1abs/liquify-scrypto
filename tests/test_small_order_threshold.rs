use scrypto_test::prelude::*;

#[derive(Clone)]
pub struct Account {
    public_key: Secp256k1PublicKey,
    account_address: ComponentAddress,
} 

pub struct TestEnvironment {
    pub ledger: LedgerSimulator<NoExtension, InMemorySubstateDatabase>,
    pub admin_account: Account,
    pub user_account1: Account,
    pub user_account2: Account,
    pub package_address: PackageAddress,
    pub liquify_component: ComponentAddress,
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
        let user_account2 = Account { public_key: user_public_key2, account_address: user_account_address2 };

        let package_address = ledger.compile_and_publish(this_package!());

        // Instantiate Liquify component
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

        let liquify_component = receipt.expect_commit(true).new_component_addresses()[0];
        let owner_badge = receipt.expect_commit(true).new_resource_addresses()[0];
        let liquidity_receipt = receipt.expect_commit(true).new_resource_addresses()[1];

        // Enable the component
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .create_proof_from_account_of_amount(
                admin_account_address, 
                owner_badge,
                1,
            )
            .call_method(
                liquify_component, 
                "set_component_status", 
                manifest_args!(true),
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

        // Setup LSUs
        let key = Secp256k1PrivateKey::from_u64(1u64).unwrap().public_key();
        let validator_address = ledger.get_active_validator_with_key(&key);
        let lsu_resource_address = ledger
            .get_active_validator_info_by_key(&key)
            .stake_unit_resource;

        // Give user1 LSUs
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet() 
            .withdraw_from_account(user_account_address1, XRD, dec!(2000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(validator_address, "stake", |lookup| {
                (lookup.bucket("xrd"),)
            })
            .call_method(
                user_account_address1,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key1)],
        );
        receipt.expect_commit_success();

        // Set minimum liquidity to 0 for testing
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

        // Set small order threshold to 100 XRD
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .create_proof_from_account_of_amount(
                admin_account_address, 
                owner_badge,
                1,
            )
            .call_method(
                liquify_component, 
                "set_small_order_threshold", 
                manifest_args!(dec!("100")),
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
   
        Self {
            ledger,
            admin_account,
            user_account1,
            user_account2,
            package_address,
            liquify_component,
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
fn test_small_order_threshold_enforcement() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let user_account2 = ledger.user_account2.account_address;
    let admin_account = ledger.admin_account.account_address;
    let liquify_component = ledger.liquify_component;
    let owner_badge = ledger.owner_badge;
    let lsu_resource_address = ledger.lsu_resource_address;

    println!("\n=== SMALL ORDER THRESHOLD TEST ===");
    println!("Small order threshold: 100 XRD");
    println!("Testing that small orders skip auto_unstake=true positions\n");

    // Set platform fee explicitly
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .create_proof_from_account_of_amount(
            admin_account, 
            owner_badge,
            1,
        )
        .call_method(
            liquify_component, 
            "set_platform_fee", 
            manifest_args!(dec!("0.0005")),  // Set to 0.05%
        )
        .call_method(
            admin_account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = ledger.execute_manifest(manifest, ledger.admin_account.clone());
    receipt.expect_commit_success();
    
    let platform_fee = dec!("0.0005"); // 0.05%
    println!("Platform fee set to: {}%", platform_fee * dec!(100));

    // Step 1: Create AUTOMATED liquidity position (auto_unstake=TRUE) with BEST discount
    println!("\nStep 1: Creating automated liquidity position (auto_unstake=true) with best discount...");
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(user_account2, XRD, dec!(200))
        .take_all_from_worktop(XRD, "xrd")
        .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
            lookup.bucket("xrd"),
            dec!("0.001"),     // Very attractive 0.1% discount
            true,              // auto_unstake = TRUE (should be skipped for small orders)
            false,             // auto_refill
            dec!("0"),         // refill_threshold
        )})
        .call_method(
            user_account2,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account2.clone());
    receipt.expect_commit_success();
    println!("✓ Created automated position with 200 XRD at 0.1% discount");

    // Step 2: Create MANUAL liquidity position (auto_unstake=FALSE) with WORSE discount
    println!("\nStep 2: Creating manual liquidity position (auto_unstake=false) with worse discount...");
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(user_account2, XRD, dec!(200))
        .take_all_from_worktop(XRD, "xrd")
        .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
            lookup.bucket("xrd"),
            dec!("0.01"),      // Worse 1% discount
            false,             // auto_unstake = FALSE (should be used for small orders)
            false,             // auto_refill
            dec!("0"),         // refill_threshold
        )})
        .call_method(
            user_account2,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account2.clone());
    receipt.expect_commit_success();
    println!("✓ Created manual position with 200 XRD at 1% discount");

    // Step 3: Test SMALL unstake (should skip automated position and use manual)
    println!("\nStep 3: Testing SMALL unstake (50 LSUs < 100 XRD threshold)...");
    println!("This should SKIP the 0.1% automated position and use the 1% manual position");
    
    let initial_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(50)  // Small amount under threshold
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            10u8
        )
        })
        .call_method(
            user_account1,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
        
    let receipt = ledger.execute_manifest(manifest, ledger.user_account1.clone());
    receipt.expect_commit_success();
    
    let final_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    let xrd_received = final_xrd - initial_xrd;
    
    // Calculate effective discount including platform fee
    let redemption_value = dec!(50);
    let expected_liquidity_discount = dec!("0.01"); // 1% from manual position (NOT 0.1%!)
    let expected_before_fee = redemption_value * (dec!(1) - expected_liquidity_discount);
    let expected_after_fee = expected_before_fee * (dec!(1) - platform_fee);
    let actual_discount = (redemption_value - xrd_received) / redemption_value;
    
    println!("XRD received: {}", xrd_received);
    println!("Effective discount: {}%", actual_discount * dec!(100));
    println!("Expected ~{}% (1% manual position + {}% platform fee), got {}%", 
        (expected_liquidity_discount + platform_fee) * dec!(100), 
        platform_fee * dec!(100),
        actual_discount * dec!(100));
    
    // Should get ~1.05% total discount (1% liquidity + 0.05% platform fee)
    let expected_total_discount = expected_liquidity_discount + platform_fee;
    assert!(
        actual_discount >= expected_total_discount * dec!("0.98") && 
        actual_discount <= expected_total_discount * dec!("1.02"),
        "Small order should have SKIPPED automated position and used manual (1% discount + {}% fee = {}%), but got {}% total",
        platform_fee * dec!(100),
        expected_total_discount * dec!(100),
        actual_discount * dec!(100)
    );
    println!("✓ Small order correctly skipped automated position and used manual position!");

    // Step 4: Test LARGE unstake (should use automated position with best discount)
    println!("\nStep 4: Testing LARGE unstake (150 LSUs > 100 XRD threshold)...");
    println!("This should use the 0.1% automated position (best discount)");
    
    let initial_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(150)  // Large amount over threshold
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            10u8
        )
        })
        .call_method(
            user_account1,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
        
    let receipt = ledger.execute_manifest(manifest, ledger.user_account1.clone());
    receipt.expect_commit_success();
    
    let final_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    let xrd_received = final_xrd - initial_xrd;
    
    // Calculate effective discount including platform fee
    let redemption_value = dec!(150);
    let expected_liquidity_discount = dec!("0.001"); // 0.1% from automated position
    let expected_before_fee = redemption_value * (dec!(1) - expected_liquidity_discount);
    let expected_after_fee = expected_before_fee * (dec!(1) - platform_fee);
    let actual_discount = (redemption_value - xrd_received) / redemption_value;
    
    println!("XRD received: {}", xrd_received);
    println!("Effective discount: {}%", actual_discount * dec!(100));
    println!("Expected ~{}% (0.1% automated position + {}% platform fee), got {}%", 
        (expected_liquidity_discount + platform_fee) * dec!(100),
        platform_fee * dec!(100),
        actual_discount * dec!(100));
    
    // Should get ~0.15% total discount (0.1% liquidity + 0.05% platform fee)
    let expected_total_discount = expected_liquidity_discount + platform_fee;
    assert!(
        actual_discount >= expected_total_discount * dec!("0.98") && 
        actual_discount <= expected_total_discount * dec!("1.02"),
        "Large order should have used automated position (0.1% discount + {}% fee = {}%), but got {}% total",
        platform_fee * dec!(100),
        expected_total_discount * dec!(100),
        actual_discount * dec!(100)
    );
    println!("✓ Large order correctly used automated position with best discount!");
    
    println!("\n✓ Small order threshold test passed!");
    println!("Small orders correctly skip auto_unstake=true positions!");
}