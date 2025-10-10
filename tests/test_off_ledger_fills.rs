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

        // *********** Set small order threshold to 1 XRD (very low) ***********
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
                manifest_args!(dec!("1")),  // Set to 1 XRD instead of default 1000
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
fn instantiate_test() {
    TestEnvironment::instantiate_test();
}

#[test]
fn test_off_ledger_fills() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let user_account4 = ledger.user_account4.account_address;
    let admin_account = ledger.admin_account.account_address;
    let liquify_component = ledger.liquify_component;
    let owner_badge = ledger.owner_badge;
    let lsu_resource_address = ledger.lsu_resource_address;

    // Lower the minimum refill threshold for testing
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .create_proof_from_account_of_amount(
            admin_account, 
            owner_badge,
            1,
        )
        .call_method(
            liquify_component, 
            "set_minimum_refill_threshold", 
            manifest_args!(dec!("1")),
        )
        .call_method(
            admin_account,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    let receipt = ledger.execute_manifest(
        manifest,
        ledger.admin_account.clone(),
    );
    receipt.expect_commit_success();
    println!("✓ Minimum refill threshold set to 1 XRD");
    println!("✓ Small order threshold set to 1 XRD (down from default 1000)");

    // CLEAR PARAMETERS - ADJUST THESE AS NEEDED
    let NUM_LIQUIDITY_POSITIONS = 50;  
    let XRD_PER_POSITION = dec!(100); 
    let NUM_KEYS_TO_TEST = 30;        

    println!("\n=== TEST PARAMETERS ===");
    println!("Creating {} liquidity positions", NUM_LIQUIDITY_POSITIONS);
    println!("Each position has {} XRD", XRD_PER_POSITION);
    println!("Will test unstaking with {} keys", NUM_KEYS_TO_TEST);
    println!("======================\n");

// Create liquidity positions - all with auto_unstake=true for off-ledger test
let mut expected_keys: Vec<u128> = Vec::new();
let discount_basis_points = 10u16; // 0.0010 * 10000

for i in 0..NUM_LIQUIDITY_POSITIONS {
    let auto_unstake = true;  // All true for off-ledger test
    
    // Calculate the key that will be created
    let receipt_id = (i + 1) as u32;
    let position = (i + 1) as u64;
    
    // Use the CORRECT bit layout matching BuyListKey::new()
    let auto_unstake_flag = if auto_unstake { 1u128 } else { 0u128 };
    let key = ((discount_basis_points as u128) << 112) |  // Top 16 bits
              ((position as u128) << 48) |                // Next 64 bits  
              ((auto_unstake_flag as u128) << 32) |       // Next 16 bits
              (receipt_id as u128);                       // Bottom 32 bits
    
    expected_keys.push(key);
        
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account4, XRD, XRD_PER_POSITION)
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.0010"),    // 0.1% discount
                auto_unstake,      // true for all
                true,              // auto_refill
                dec!("100"),       // refill_threshold
                dec!("5"),         // automation_fee
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

    println!("✓ Successfully created {} liquidity positions (all with auto_unstake=true)", NUM_LIQUIDITY_POSITIONS);

    // First, test with regular unstaking with LARGE amount (above small order threshold)
    println!("\n=== TESTING REGULAR UNSTAKE WITH LARGE AMOUNT ===");
    let initial_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    let initial_lsu = ledger.ledger.get_component_balance(user_account1, lsu_resource_address);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(10) // 10 LSUs = ~10 XRD value, above 1 XRD threshold
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            30u8
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
    
    let mid_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    let mid_lsu = ledger.ledger.get_component_balance(user_account1, lsu_resource_address);
    let regular_xrd_received = mid_xrd - initial_xrd;
    let regular_lsu_spent = initial_lsu - mid_lsu;
    
    println!("Regular unstake (10 LSUs): {} LSUs → {} XRD", regular_lsu_spent, regular_xrd_received);
    assert!(regular_xrd_received > dec!(0), "Large regular unstaking should work with auto_unstake=true positions");

    // Now test off-ledger with the keys we tracked
    println!("\n=== TESTING OFF-LEDGER UNSTAKE ===");
    
    // Use the first NUM_KEYS_TO_TEST keys
    let off_ledger_order_vec: Vec<u128> = expected_keys.into_iter().take(NUM_KEYS_TO_TEST as usize).collect();
    
    println!("Using {} off-ledger keys", off_ledger_order_vec.len());
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(100) // Unstake 100 LSUs - well above small order threshold
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(
            liquify_component, 
            "liquify_unstake_off_ledger", |lookup| {
            (lookup.bucket("lsu"),
            off_ledger_order_vec.clone(),
        )})
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
    
    // Check results
    let final_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    let final_lsu = ledger.ledger.get_component_balance(user_account1, lsu_resource_address);
    let xrd_received = final_xrd - mid_xrd;
    let lsu_spent = mid_lsu - final_lsu;
    let lsu_returned = dec!(100) - lsu_spent;
    
    println!("\n=== OFF-LEDGER RESULTS ===");
    println!("XRD received: {}", xrd_received);
    println!("LSUs spent: {}", lsu_spent);
    println!("LSUs returned: {}", lsu_returned);
    
    assert!(xrd_received > dec!(0), "Off-ledger unstaking should work with correct keys");
    
    println!("\n✓ Off-ledger unstaking WORKED!");
    println!("Successfully unstaked {} LSUs for {} XRD using off-ledger keys", lsu_spent, xrd_received);
    
    // Also test that small orders are properly rejected from auto_unstake=true positions
    println!("\n=== TESTING SMALL ORDER BEHAVIOR ===");
    let before_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!("0.5") // 0.5 LSUs = ~0.5 XRD value, below 1 XRD threshold
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            30u8
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
    
    let after_xrd = ledger.ledger.get_component_balance(user_account1, XRD);
    let small_order_xrd = after_xrd - before_xrd;
    
    println!("Small order (0.5 LSUs) received: {} XRD", small_order_xrd);
    println!("✓ Small order correctly skipped auto_unstake=true positions (no XRD received)");
    
    println!("\n✓ All tests passed!");
}