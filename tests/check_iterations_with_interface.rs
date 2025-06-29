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
    pub liquify_interface_component: ComponentAddress,
    pub owner_badge: ResourceAddress,
    pub interface_owner_badge: ResourceAddress,
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
        let interface_owner_badge = receipt.expect_commit(true).new_resource_addresses()[0];

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

        // *********** Set interface target WITH OWNER BADGE ***********
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .create_proof_from_account_of_amount(
                admin_account_address, 
                interface_owner_badge,
                1,
            )
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

        // *********** Set minimum refill threshold to 1 ***********
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
                manifest_args!(dec!("1")),
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
                let refill_threshold = if auto_refill { dec!("10") } else { dec!("0") };
                
                let manifest = ManifestBuilder::new()
                    .lock_fee_from_faucet()
                    .withdraw_from_account(user_account_address4, XRD, dec!(1))
                    .take_all_from_worktop(XRD, "xrd")
                    .call_method_with_name_lookup(interface_component, "add_liquidity", |lookup| {(
                        lookup.bucket("xrd"),
                        discount,
                        auto_unstake,
                        auto_refill,
                        refill_threshold,
                        dec!("5"),  // automation_fee - ADD THIS LINE
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
            interface_owner_badge,
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

    println!("=== TEST PARAMETERS ===");
    println!("Testing interface with 30 max iterations");
    println!("90 liquidity positions created (30 each at 1%, 2%, 2.5% discount)");
    println!("======================\n");

    // Get initial balances
    let initial_xrd_balance = ledger.ledger.get_component_balance(user_account1, XRD);
    let initial_lsu_balance = ledger.ledger.get_component_balance(user_account1, lsu_resource_address);
    println!("=== INITIAL BALANCES ===");
    println!("User1 initial XRD: {} XRD", initial_xrd_balance);
    println!("User1 initial LSU: {} LSU", initial_lsu_balance);

    // Test unstaking through interface with iteration limit
    let lsu_to_unstake = dec!(1000);
    println!("\n=== UNSTAKING TEST ===");
    println!("Attempting to unstake {} LSUs through interface", lsu_to_unstake);
    println!("Max iterations: 30");
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            lsu_to_unstake
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_interface_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            30u8  // max_iterations for interface
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
    
    // Check if transaction succeeded
    if receipt.is_commit_success() {
        println!("\n✓ SUCCESS: Interface unstaking completed!");
        
        // Calculate results
        let final_xrd_balance = ledger.ledger.get_component_balance(user_account1, XRD);
        let final_lsu_balance = ledger.ledger.get_component_balance(user_account1, lsu_resource_address);
        let xrd_received = final_xrd_balance - initial_xrd_balance;
        let lsu_spent = initial_lsu_balance - final_lsu_balance;
        
        println!("\n=== RESULTS ===");
        println!("XRD received: {} XRD", xrd_received);
        println!("LSUs spent: {} LSU", lsu_spent);
        println!("LSUs returned: {} LSU", lsu_to_unstake - lsu_spent);
        
        // Calculate effective discount
        if lsu_spent > dec!(0) {
            let redemption_value = lsu_spent; // Assuming 1:1 redemption rate
            let effective_discount = (redemption_value - xrd_received) / redemption_value * dec!(100);
            println!("Effective discount: {}%", effective_discount);
        }
        
        println!("\n=== FINAL BALANCES ===");
        println!("User1 final XRD: {} XRD", final_xrd_balance);
        println!("User1 final LSU: {} LSU", final_lsu_balance);
        
        // Show approximate positions filled
        if lsu_spent > dec!(0) {
            let approx_positions = lsu_spent / dec!(1);  // Changed from 10 to 1 since positions are 1 XRD each
            println!("\nApproximate positions filled: {} (assuming 1 XRD positions)", approx_positions);
        }
        
        assert!(xrd_received > dec!(0), "Should have received XRD from unstaking");
        assert!(lsu_spent > dec!(0), "Should have spent LSUs");
    } else {
        println!("\n✗ FAILED: Interface unstaking failed!");
        println!("Error: {:?}", receipt.expect_rejection());
    }
}