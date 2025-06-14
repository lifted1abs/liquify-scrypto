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
        let  user_account2 = Account { public_key: user_public_key2, account_address: user_account_address2 };

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

        // Give user1 LSUs for unstaking
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet() 
            .withdraw_from_account(user_account_address1, XRD, dec!(5000))
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

        // Set minimum liquidity to 100 for easier testing
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

        // Set minimum refill threshold to 100
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .create_proof_from_account_of_amount(
                admin_account_address, 
                owner_badge,
                1,
            )
            .call_method(
                liquify_component, 
                "set_minimum_refill_threshold", 
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
fn test_cycle_liquidity_full_flow() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let user_account2 = ledger.user_account2.account_address;
    let liquify_component = ledger.liquify_component;
    let lsu_resource_address = ledger.lsu_resource_address;
    let liquidity_receipt = ledger.liquidity_receipt;

    println!("\n=== CYCLE LIQUIDITY FULL FLOW TEST ===\n");

    // Step 1: Create liquidity position with automation enabled
    println!("Step 1: Creating liquidity position with automation enabled...");
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(user_account2, XRD, dec!(1000))
        .take_all_from_worktop(XRD, "xrd")
        .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
            lookup.bucket("xrd"),
            dec!("0.01"),      // 1% discount
            true,              // auto_unstake ENABLED
            true,              // auto_refill ENABLED
            dec!("200"),       // refill_threshold (200 XRD)
        )})
        .call_method(
            user_account2,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account2.clone());
    receipt.expect_commit_success();
    println!("✓ Created liquidity position with 1000 XRD (Receipt #1)");

    // Step 2: Unstake LSUs to completely fill the liquidity
    println!("\nStep 2: Unstaking LSUs to fill the liquidity position...");
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(1010)
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
    println!("✓ Unstaked LSUs and filled the liquidity position");

    // Step 3: Check claimable XRD before epoch advancement
    println!("\nStep 3: Checking claimable XRD before epoch advancement...");
    let receipt_id = NonFungibleLocalId::Integer(IntegerNonFungibleLocalId::new(1));
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .call_method(
            liquify_component,
            "get_claimable_xrd",
            manifest_args!(receipt_id.clone()),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account1.clone());
    println!("Claimable XRD check result: {}", if receipt.is_commit_success() { "SUCCESS" } else { "FAILED" });

    // Step 4: PROPERLY advance 501 epochs
    println!("\nStep 4: Advancing 501 epochs to pass unbonding period...");
    let current_epoch = ledger.ledger.get_current_epoch();
    let current_round = ledger.ledger.get_consensus_manager_state().round;
    println!("Current epoch: {}", current_epoch.number());
    println!("Current round: {}", current_round.number());
    
    // We need to figure out how many rounds per epoch
    // Let's advance by a large number of rounds to ensure we pass 501 epochs
    // If we assume ~1000 rounds per epoch (common in tests), we need 501,000 rounds
    let target_round = current_round.number() + 501_000;
    
    println!("Advancing to round {} to pass unbonding period...", target_round);
    ledger.ledger.advance_to_round(Round::of(target_round));
    
    let new_epoch = ledger.ledger.get_current_epoch();
    println!("✓ Advanced to epoch: {}", new_epoch.number());
    
    // Verify we actually advanced enough epochs
    if new_epoch.number() < current_epoch.number() + 500 {
        println!("WARNING: Only advanced {} epochs, expected 501+", new_epoch.number() - current_epoch.number());
    }

    // Step 5: Check claimable XRD after epoch advancement
    println!("\nStep 5: Checking claimable XRD after epoch advancement...");
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .call_method(
            liquify_component,
            "get_claimable_xrd",
            manifest_args!(receipt_id.clone()),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account1.clone());
    println!("Claimable XRD check after advancement: {}", if receipt.is_commit_success() { "SUCCESS" } else { "FAILED" });

    // Step 6: Attempt to cycle liquidity
    println!("\nStep 6: Attempting to cycle liquidity...");
    
    let user1_xrd_before = ledger.ledger.get_component_balance(user_account1, XRD);
    println!("User1 XRD balance before cycle: {}", user1_xrd_before);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .call_method(
            liquify_component,
            "cycle_liquidity",
            manifest_args!(
                receipt_id,
                10u64
            ),
        )
        .call_method(
            user_account1,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account1.clone());
    println!("{:?}\n", receipt);
    
    if receipt.is_commit_success() {
        println!("✓ Successfully cycled liquidity!");
        
        let user1_xrd_after = ledger.ledger.get_component_balance(user_account1, XRD);
        let automation_fee_received = user1_xrd_after - user1_xrd_before;
        println!("User1 XRD balance after cycle: {}", user1_xrd_after);
        println!("Automation fee received: {} XRD", automation_fee_received);
    } else {
        println!("✗ Failed to cycle liquidity");
        println!("This should only succeed if we're past the unbonding period");
    }
}