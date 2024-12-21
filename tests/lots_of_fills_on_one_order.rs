use scrypto_test::prelude::*;

#[derive(Clone)]
pub struct Account {
    public_key: Secp256k1PublicKey,
    account_address: ComponentAddress,
} 

pub struct TestEnvironment {

    ledger: LedgerSimulator<NoExtension, InMemorySubstateDatabase>,
    admin_account: Account,
    user_account1: Account,
    user_account2: Account,
    user_account3: Account,
    user_account4: Account,
    user_account5: Account,
    user_account6: Account,
    package_address: PackageAddress,
    liquify_component: ComponentAddress,
    owner_badge: ResourceAddress,
    liquidity_receipt: ResourceAddress,

    lsu_resource_address: ResourceAddress,
}

impl TestEnvironment {
    pub fn instantiate_test() -> Self {

        let custom_genesis = CustomGenesis::default(
            Epoch::of(1),
            CustomGenesis::default_consensus_manager_config(),
        );
        let mut ledger = LedgerSimulatorBuilder::new()
            .with_custom_genesis(custom_genesis)
            .without_kernel_trace()
            .build();

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
            .deposit_batch(admin_account_address)
            .build();
        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&admin_public_key)],
        );
        println!("{:?}\n", receipt);

        let liquify_component = receipt.expect_commit(true).new_component_addresses()[0];
        let owner_badge = receipt.expect_commit(true).new_resource_addresses()[0];
        let liquidity_receipt = receipt.expect_commit(true).new_resource_addresses()[1];

        // *********** User 1 stakes 1000 XRD to validator to receive LSUs ***********

        let key = Secp256k1PrivateKey::from_u64(1u64).unwrap().public_key();
        let validator_address = ledger.get_active_validator_with_key(&key);
        let lsu_resource_address = ledger
            .get_active_validator_info_by_key(&key)
            .stake_unit_resource;

        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet() 
            .withdraw_from_account(user_account_address1, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(validator_address, "stake", |lookup| {
                (lookup.bucket("xrd"),)
            })
            .deposit_batch(user_account_address1)
            .build();

        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key1)],
        );
        let commit_success = receipt.expect_commit_success();

        // *********** User 2 stakes 1000 XRD to validator to receive LSUs ***********

        let lsu_resource_address = ledger
            .get_active_validator_info_by_key(&key)
            .stake_unit_resource;

        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet() 
            .withdraw_from_account(user_account_address2, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(validator_address, "stake", |lookup| {
                (lookup.bucket("xrd"),)
            })
            .deposit_batch(user_account_address2)
            .build();

        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key2)],
        );
        let commit_success = receipt.expect_commit_success();

        // *********** User 3 stakes 1000 XRD to validator to receive LSUs ***********

        let lsu_resource_address = ledger
            .get_active_validator_info_by_key(&key)
            .stake_unit_resource;

        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet() 
            .withdraw_from_account(user_account_address3, XRD, dec!(1000))
            .take_all_from_worktop(XRD, "xrd")
            .call_method_with_name_lookup(validator_address, "stake", |lookup| {
                (lookup.bucket("xrd"),)
            })
            .deposit_batch(user_account_address3)
            .build();

        let receipt = ledger.execute_manifest(
            manifest,
            vec![NonFungibleGlobalId::from_public_key(&user_public_key3)],
        );
        let commit_success = receipt.expect_commit_success();

        // let lsu = commit_success.new_resource_addresses()[0];
        let account1_lsu_balance = ledger.get_component_balance(
            user_account_address1, 
            lsu_resource_address
        );

        println!("lsu_address {:?}", lsu_resource_address);
        println!("account1_lsu_ amount {:?}", account1_lsu_balance);

        // *********** set min buy order to 0 for these tests***********

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

        // *********** User 4 creates a buy order for 1000 XRD at 0.1% discount from face value ***********
        
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account_address4, XRD, dec!(900))
            .take_all_from_worktop(XRD, "xrd")
            
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.0010"),
                true,
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

        // *********** User 5 creates a buy order for 1000 XRD at 1% discount from face value ***********
        
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account_address5, XRD, dec!(900))
            .take_all_from_worktop(XRD, "xrd")
            
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.010"),
                true,
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

        // *********** User 6 creates a buy order for 1000 XRD at 5% discount from face value ***********
        
        let manifest = ManifestBuilder::new()
            .lock_fee_from_faucet()
            .withdraw_from_account(user_account_address6, XRD, dec!(900))
            .take_all_from_worktop(XRD, "xrd")
            
            .call_method_with_name_lookup(liquify_component, "add_liquidity", |lookup| {(
                lookup.bucket("xrd"),
                dec!("0.050"),
                true,
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
    // Part 1 Set up test
    TestEnvironment::instantiate_test();
}





#[test]
fn instantiate_test2() {
    // Part 1 Set up test
    let mut ledger = TestEnvironment::instantiate_test();
    let admin_account = ledger.admin_account.account_address;
    let user_account1 = ledger.user_account1.account_address;
    let user_account2 = ledger.user_account2.account_address;
    let user_account3 = ledger.user_account3.account_address;
    let user_account4 = ledger.user_account4.account_address;
    let user_account5 = ledger.user_account5.account_address;
    let user_account6 = ledger.user_account6.account_address;
    let package_address = ledger.package_address;
    let liquify_component = ledger.liquify_component;
    let owner_badge = ledger.owner_badge;
    let liquidity_receipt = ledger.liquidity_receipt;
    let lsu_resource_address = ledger.lsu_resource_address;


    // can handle up to 85 at a time, any more will be saved for the next time user collects fills
    for _ in 0..89 {

        // *********** User 1 sells 10 LSUs at market rate LSUs ***********
        let manifest = ManifestBuilder::new()
            .lock_fee(user_account1, 50)
            .withdraw_from_account(
                user_account1, 
                lsu_resource_address, 
                dec!(1)
            )
            .take_all_from_worktop(lsu_resource_address, "lsu")
            .call_method_with_name_lookup(liquify_component, "liquify_unstake", |lookup| {
                (lookup.bucket("lsu"),)
            })
            .call_method(
                user_account1,
                "deposit_batch",
                manifest_args!(ManifestExpression::EntireWorktop),
            )
            .deposit_batch(user_account1);
        
        let receipt = ledger.execute_manifest(
            manifest.build(),
            ledger.user_account1.clone(),
        );
        println!("{:?}\n", receipt);
        let commit = receipt.expect_commit_success();

    }



    // // ********** User 4 collects unstake nft **********
   
    let manifest = ManifestBuilder::new()
        .lock_fee_from_faucet()
        .withdraw_from_account(
            user_account4, 
            liquidity_receipt, 
            // account1_lsu_balance
            dec!(1)
        )
        .take_all_from_worktop(liquidity_receipt, "liquidity_receipt_bucket")
        
        .call_method_with_name_lookup(liquify_component, "collect_fills", |lookup| {
            (lookup.bucket("liquidity_receipt_bucket"),)
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
