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
    pub user_account3: Account,
    pub user_account4: Account,
    pub user_account5: Account,
    pub user_account6: Account,
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

        let (user_public_key3, _user_private_key3, user_account_address3) = ledger.new_allocated_account();
        let  user_account3 = Account { public_key: user_public_key3, account_address: user_account_address3 };

        let (user_public_key4, _user_private_key4, user_account_address4) = ledger.new_allocated_account();
        let  user_account4 = Account { public_key: user_public_key4, account_address: user_account_address4 };

        let (user_public_key5, _user_private_key5, user_account_address5) = ledger.new_allocated_account();
        let  user_account5 = Account { public_key: user_public_key5, account_address: user_account_address5 };

        let (user_public_key6, _user_private_key6, user_account_address6) = ledger.new_allocated_account();
        let  user_account6 = Account { public_key: user_public_key6, account_address: user_account_address6 };

        let package_address = ledger.compile_and_publish(this_package!());

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

        let liquify_component = receipt.expect_commit(true).new_component_addresses()[0];
        let owner_badge = receipt.expect_commit(true).new_resource_addresses()[0];
        let liquidity_receipt = receipt.expect_commit(true).new_resource_addresses()[1];

        // *********** Enable the component (it starts disabled) ***********
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

        // *********** Set minimum liquidity to 0 for testing ***********
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
fn test_platform_fees() {
    let mut ledger = TestEnvironment::instantiate_test();
    let admin_account_address = ledger.admin_account.account_address;
    let user_account1 = ledger.user_account1.account_address;
    let user_account4 = ledger.user_account4.account_address;
    let liquify_component = ledger.liquify_component;
    let owner_badge = ledger.owner_badge;
    let lsu_resource_address = ledger.lsu_resource_address;

    // CLEAR PARAMETERS
    let PLATFORM_FEE = dec!("0.005");  // 0.5% platform fee
    let LIQUIDITY_AMOUNT = dec!(1000); // 1000 XRD per position
    let NUM_POSITIONS = 5;
    let UNSTAKE_AMOUNT = dec!(1000);  // LSUs to unstake

    println!("=== PLATFORM FEE TEST ===");
    println!("Platform fee: {}%", PLATFORM_FEE * dec!(100));
    println!("Creating {} positions with {} XRD each", NUM_POSITIONS, LIQUIDITY_AMOUNT);
    println!("Will unstake {} LSUs", UNSTAKE_AMOUNT);
    println!("========================\n");

    // Set platform fee
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .create_proof_from_account_of_amount(
            admin_account_address, 
            owner_badge,
            1,
        )
        .call_method(
            liquify_component, 
            "set_platform_fee", 
            manifest_args!(PLATFORM_FEE),
        )
        .call_method(
            admin_account_address,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = ledger.execute_manifest(
        manifest,
        ledger.admin_account.clone(),
    );
    receipt.expect_commit_success();
    println!("✓ Platform fee set to {}%", PLATFORM_FEE * dec!(100));

    // Create liquidity positions
    for i in 0..NUM_POSITIONS {
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account4, XRD, LIQUIDITY_AMOUNT)
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.01"),     // 1% discount
                false,            // auto_unstake
                false,            // auto_refill
                dec!("0"),        // refill_threshold
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
        receipt.expect_commit_success();
    }
    println!("✓ Created {} liquidity positions", NUM_POSITIONS);

    // Track XRD balance before unstaking
    let user1_xrd_before = ledger.ledger.get_component_balance(user_account1, XRD);
    println!("\nUser1 XRD before unstaking: {}", user1_xrd_before);

    // Perform unstaking
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            UNSTAKE_AMOUNT
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            30u8  // max_iterations
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
    receipt.expect_commit_success();

    // Calculate actual amounts
    let user1_xrd_after = ledger.ledger.get_component_balance(user_account1, XRD);
    let xrd_received = user1_xrd_after - user1_xrd_before;
    
    println!("\n=== UNSTAKING RESULTS ===");
    println!("XRD received by user: {}", xrd_received);
    
    // Calculate expected values
    // With 1% discount, user pays 990 XRD for 1000 LSUs
    // Platform fee is 0.5% of 990 = 4.95 XRD
    let liquidity_used = dec!(990); // 1000 LSUs with 1% discount
    let expected_platform_fee = liquidity_used * PLATFORM_FEE;
    let expected_user_receives = liquidity_used - expected_platform_fee;
    
    println!("\nExpected platform fee: {} XRD", expected_platform_fee);
    println!("Expected user receives: {} XRD", expected_user_receives);
    println!("Actual user received: {} XRD", xrd_received);
    
    // Verify fee calculation (check if difference is small)
    let fee_difference = if expected_user_receives > xrd_received {
        expected_user_receives - xrd_received
    } else {
        xrd_received - expected_user_receives
    };
    assert!(fee_difference < dec!("0.001"), "Fee calculation mismatch! Expected {} but got {}", expected_user_receives, xrd_received);

    // Collect platform fees
    let admin_xrd_before = ledger.ledger.get_component_balance(admin_account_address, XRD);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .create_proof_from_account_of_amount(
            admin_account_address, 
            owner_badge,
            1,
        )
        .call_method(
            liquify_component, 
            "collect_platform_fees", 
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
        ledger.admin_account.clone(),
    );
    receipt.expect_commit_success();

    let admin_xrd_after = ledger.ledger.get_component_balance(admin_account_address, XRD);
    let fees_collected = admin_xrd_after - admin_xrd_before;
    
    println!("\n=== FEE COLLECTION ===");
    println!("Fees collected: {} XRD", fees_collected);
    println!("Expected fees: {} XRD", expected_platform_fee);
    
    // Verify collected fees match expected
    let collected_difference = if fees_collected > expected_platform_fee {
        fees_collected - expected_platform_fee
    } else {
        expected_platform_fee - fees_collected
    };
    assert!(collected_difference < dec!("0.001"), "Collected fees mismatch! Expected {} but got {}", expected_platform_fee, fees_collected);
    
    println!("\n✓ Platform fee test passed!");
}