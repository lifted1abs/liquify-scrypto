use scrypto_test::prelude::*;
use std::time::Instant;

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

        // Give users more LSUs for scalability testing
        for (user_account, user_public_key) in [
            (user_account_address1, user_public_key1),
            (user_account_address2, user_public_key2),
            (user_account_address3, user_public_key3),
        ] {
            // Load account with XRD first
            ledger.load_account_from_faucet(user_account);
            
            let manifest = ManifestBuilder::new()
                .lock_fee_from_faucet() 
                .withdraw_from_account(user_account, XRD, dec!(5000))
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

        // Load liquidity provider accounts
        ledger.load_account_from_faucet(user_account_address4);
        ledger.load_account_from_faucet(user_account_address5);
        ledger.load_account_from_faucet(user_account_address6);

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
fn test_add_liquidity_scaling() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let user_account4 = ledger.user_account4.account_address;
    let liquify_component = ledger.liquify_component;

    // PARAMETER - Change this value to test different tree sizes
    let TREE_SIZE: usize = 100; // Try: 0, 10, 50, 100, 500, 1000

    println!("\n=== ADD LIQUIDITY SCALING TEST ===");
    println!("Building AVL tree with {} positions...", TREE_SIZE);

    // Populate the AVL tree
    for i in 0..TREE_SIZE {
        if i % 50 == 0 && i > 0 {
            ledger.ledger.load_account_from_faucet(user_account4);
        }

        let discount = match i % 5 {
            0 => dec!("0.001"),
            1 => dec!("0.005"),
            2 => dec!("0.010"),
            3 => dec!("0.015"),
            _ => dec!("0.020"),
        };

        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account4, XRD, dec!(1))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                discount,
                false,          // auto_unstake
                false,          // auto_refill
                dec!("0"),      // refill_threshold
                dec!("5"),       // automation_fee
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
            println!("Failed to add position {}", i);
            break;
        }
    }

    println!("✓ Tree populated with {} positions\n", TREE_SIZE);

    // Test add liquidity with full tree
    println!("=== TESTING ADD_LIQUIDITY WITH {} POSITIONS ===", TREE_SIZE);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(user_account1, XRD, dec!(10))
        .take_all_from_worktop(XRD, "xrd")
        .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
            lookup.bucket("xrd"),
            dec!("0.01"),       // 1% discount
            false,              // auto_unstake
            false,              // auto_refill
            dec!("0"),          // refill_threshold
            dec!("5"),       // automation_fee
        )})
        .call_method(
            user_account1,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account1.clone());
    
    println!("\n=== RESULT ===");
    println!("Tree size: {} positions", TREE_SIZE);
    println!("Transaction: {}", if receipt.is_commit_success() { "SUCCESS" } else { "FAILED" });
    println!("\nLook for 'Transaction Cost:' in the output above");
    println!("\nTo test scaling, run with different TREE_SIZE values:");
    println!("0, 10, 50, 100, 500, 1000");
}

#[test]
fn test_unstake_scaling() {
    let mut ledger = TestEnvironment::instantiate_test();
    let user_account1 = ledger.user_account1.account_address;
    let user_account4 = ledger.user_account4.account_address;
    let liquify_component = ledger.liquify_component;
    let lsu_resource_address = ledger.lsu_resource_address;

    // PARAMETER - Change this value to test different tree sizes
    let TREE_SIZE: usize = 100; // Try: 0, 10, 50, 100, 500, 1000

    println!("\n=== UNSTAKE SCALING TEST ===");
    println!("Building AVL tree with {} positions...", TREE_SIZE);

    // Populate the AVL tree
    for i in 0..TREE_SIZE {
        if i % 50 == 0 && i > 0 {
            ledger.ledger.load_account_from_faucet(user_account4);
        }

        let discount = match i % 5 {
            0 => dec!("0.001"),
            1 => dec!("0.005"),
            2 => dec!("0.010"),
            3 => dec!("0.015"),
            _ => dec!("0.020"),
        };

        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account4, XRD, dec!(1))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                discount,
                false,          // auto_unstake
                false,          // auto_refill
                dec!("0"),      // refill_threshold
                dec!("5"),       // automation_fee
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
            println!("Failed to add position {}", i);
            break;
        }
    }

    println!("✓ Tree populated with {} positions\n", TREE_SIZE);

    // Test unstaking with full tree
    println!("=== TESTING LIQUIFY_UNSTAKE WITH {} POSITIONS ===", TREE_SIZE);
    
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account1, 
            lsu_resource_address, 
            dec!(10)
        )
        .take_all_from_worktop(lsu_resource_address, "lsu")
        .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
            (lookup.bucket("lsu"),
            10u8  // max_iterations
        )
        })
        .call_method(
            user_account1,
            "deposit_batch",
            manifest_args!(ManifestExpression::EntireWorktop),
        )
        .build();
    
    let receipt = ledger.execute_manifest(manifest, ledger.user_account1.clone());

    println!("\n=== RESULT ===");
    println!("Tree size: {} positions", TREE_SIZE);
    println!("Max iterations: 10");
    println!("Transaction: {}", if receipt.is_commit_success() { "SUCCESS" } else { "FAILED" });
    println!("\nLook for 'Transaction Cost:' in the output above");
    println!("\nTo test scaling, run with different TREE_SIZE values:");
    println!("0, 10, 50, 100, 500, 1000");
}