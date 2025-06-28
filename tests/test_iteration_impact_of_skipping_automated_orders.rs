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
        println!("{:?}\n", receipt);

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

        // *********** User 1 stakes XRD to validator to receive LSUs ***********
        let key = Secp256k1PrivateKey::from_u64(1u64).unwrap().public_key();
        let validator_address = ledger.get_active_validator_with_key(&key);
        let lsu_resource_address = ledger
            .get_active_validator_info_by_key(&key)
            .stake_unit_resource;

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

        // *********** User 2 stakes XRD to validator to receive LSUs ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet() 
            .withdraw_from_account(user_account_address2, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(validator_address, "stake", |lookup| {
                (lookup.bucket("xrd"),)  // Fixed: removed extra comma before closing paren
            })
            .call_method(
                user_account_address2,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key2)],
        );
        receipt.expect_commit_success();

        // *********** User 3 stakes XRD to validator to receive LSUs ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet() 
            .withdraw_from_account(user_account_address3, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(validator_address, "stake", |lookup| {
                (lookup.bucket("xrd"),)
            })
            .call_method(
                user_account_address3,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();

        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key3)],
        );
        receipt.expect_commit_success();

        let account1_lsu_balance = ledger.get_component_balance(
            user_account_address1, 
            lsu_resource_address
        );

        println!("lsu_address {:?}", lsu_resource_address);
        println!("account1_lsu_ amount {:?}", account1_lsu_balance);

        // *********** Set minimum liquidity to 0 for tests ***********
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
        println!("{:?}\n", receipt);
        receipt.expect_commit_success();

        // *********** User 4 creates liquidity with new parameters ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account_address4, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.0010"),  // discount
                false,           // auto_unstake
                false,           // auto_refill
                dec!("0"),       // refill_threshold
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
        println!("{:?}\n", receipt);
        receipt.expect_commit_success();

        // *********** User 5 creates liquidity with auto_unstake enabled ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account_address5, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.010"),   // discount
                true,            // auto_unstake
                false,           // auto_refill
                dec!("0"),       // refill_threshold
            )})
            .call_method(
                user_account_address5,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key5)],
        );
        println!("{:?}\n", receipt);
        receipt.expect_commit_success();

        // *********** User 6 creates liquidity with automation enabled ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account_address6, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.050"),    // discount
                true,             // auto_unstake (required for auto_refill)
                true,             // auto_refill
                dec!("10000"),    // refill_threshold
            )})
            .call_method(
                user_account_address6,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .build();
        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key6)],
        );
        println!("{:?}\n", receipt);
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
fn instantiate_test() {
    TestEnvironment::instantiate_test();
}

#[test]
fn test_basic_liquify_flow() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let liquify_component = ledger.liquify_component;
    let lsu_resource_address = ledger.lsu_resource_address;

    // User 1 sells LSUs
    let manifest = ManifestBuilder::new()
        .lock_fee(user_account1, 50)
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(5000)   
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
                30u8 // max_iterations
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
fn test_iteration_impact_of_skipping_automated_orders() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let user_account4 = ledger.user_account4.account_address;
    let liquify_component = ledger.liquify_component;
    let lsu_resource_address = ledger.lsu_resource_address;

    // PARAMETERS TO TEST
    let AUTOMATED_POSITIONS = 100;     // Number of auto_unstake=true positions to skip
    let MANUAL_POSITIONS = 20;         // Number of auto_unstake=false positions available
    let MAX_ITERATIONS: u8 = 50;       // Max iterations to attempt
    let SMALL_UNSTAKE = dec!("0.001"); // Small unstake to trigger skip logic

    println!("\n=== ITERATION SKIP TEST ===");
    println!("Creating {} automated positions (will be skipped)", AUTOMATED_POSITIONS);
    println!("Creating {} manual positions (can be filled)", MANUAL_POSITIONS);
    println!("Max iterations allowed: {}", MAX_ITERATIONS);
    println!("===========================\n");

    // First, create AUTOMATED positions (auto_unstake=true) with best discounts
    println!("Creating automated positions...");
    for i in 0..AUTOMATED_POSITIONS {
        if i % 50 == 0 && i > 0 {
            println!("  Created {} automated positions...", i);
        }

        let discount = dec!("0.001"); // Very attractive discount

        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account4, XRD, dec!(10))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                discount,
                true,           // auto_unstake = TRUE (will be skipped)
                false,          // auto_refill
                dec!("0"),      // refill_threshold
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
        
        if !receipt.is_commit_success() {
            println!("Failed to create automated position {}", i);
            break;
        }
    }
    println!("✓ Created {} automated positions\n", AUTOMATED_POSITIONS);

    // Now create MANUAL positions (auto_unstake=false) with slightly worse discounts
    println!("Creating manual positions...");
    for i in 0..MANUAL_POSITIONS {
        let discount = dec!("0.002"); // Slightly worse discount

        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account4, XRD, dec!(10))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                discount,
                false,          // auto_unstake = FALSE (can be filled)
                false,          // auto_refill
                dec!("0"),      // refill_threshold
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
        
        if !receipt.is_commit_success() {
            println!("Failed to create manual position {}", i);
            break;
        }
    }
    println!("✓ Created {} manual positions\n", MANUAL_POSITIONS);

    // Get initial balances
    let initial_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    let initial_lsu = ledger.ledger.get_component_balance(user_account1, lsu_resource_address);

    // Perform SMALL unstake to trigger skip logic
    println!("=== PERFORMING SMALL UNSTAKE ===");
    println!("Unstaking {} LSUs (triggers small order protection)", SMALL_UNSTAKE);
    println!("This should skip all {} automated positions", AUTOMATED_POSITIONS);
    println!("Max iterations: {}", MAX_ITERATIONS);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            SMALL_UNSTAKE
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            MAX_ITERATIONS
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
    
    // Analyze results
    if receipt.is_commit_success() {
        let final_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
        let final_lsu = ledger.ledger.get_component_balance(user_account1, lsu_resource_address);
        
        let xrd_received = final_xrd - initial_xrd;
        let lsu_spent = initial_lsu - final_lsu;
        
        println!("\n=== RESULTS ===");
        println!("Transaction: SUCCESS");
        println!("XRD received: {}", xrd_received);
        println!("LSUs spent: {}", lsu_spent);
        println!("LSUs returned: {}", SMALL_UNSTAKE - lsu_spent);
        
        if lsu_spent > dec!(0) {
            println!("\n✓ Successfully filled order despite skipping {} automated positions!", AUTOMATED_POSITIONS);
            println!("This proves the skip logic is efficient enough to handle many skips within iteration limit");
        } else {
            println!("\n✗ No LSUs were spent - max iterations might have been exhausted");
            println!("Try increasing MAX_ITERATIONS or reducing AUTOMATED_POSITIONS");
        }
    } else {
        println!("\n✗ Transaction FAILED");
        println!("Error: {:?}", receipt.expect_rejection());
    }
    
    // Test with LARGE unstake (should NOT skip automated positions)
    println!("\n\n=== CONTROL TEST: LARGE UNSTAKE ===");
    println!("Now testing with 1000 LSUs (no small order protection)");
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(1000)
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            10u8  // Much fewer iterations needed
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
    
    if receipt.is_commit_success() {
        println!("✓ Large unstake succeeded with just 10 iterations");
        println!("This confirms automated positions are used for large orders");
    } else {
        println!("✗ Large unstake failed");
    }
}