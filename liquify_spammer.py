import secrets
import sys
import requests
import json
from datetime import datetime
from radix_engine_toolkit import *
import os
import asyncio
import random
from pathlib import Path

# Endpoints for the Gateway API
BASE_URL = "https://stokenet.radixdlt.com"

DEV_ADDRESS = "account_tdx_2_12yqz8dp4gu5wn70k66qqncuhh0hny77tag3ync44w0yrshpxumrj55"

# Bot config
CREDS_FILENAME = "creds.json"
NETWORK = "stokenet"
NETWORK_NUMBER = 2 # Mainnet = 1, Stokenet = 2

class SpammerInfo:
    def __init__(
            self,
            account: Address = None,
            base_url: str = "",
            current_epoch: int = 0,
            min_to_receive: Decimal = Decimal("0"),
            network: str = "mainnet",
            network_number: int = 1,
            private_key: PrivateKey = None,
            public_key: PublicKey = None,
            spam_message: str = "",
            session: requests.Session = requests.Session(),
            config: dict = None
    ):
        self.account = account
        self.base_url = base_url
        self.current_epoch = current_epoch
        self.min_to_receive = min_to_receive
        self.network = network
        self.network_number = network_number
        self.private_key = private_key
        self.public_key = public_key
        self.spam_message = spam_message
        self.session = session
        self.config = config

async def load_config():
    """Load configuration from the config file."""
    config_path = Path(__file__).parent / f"{NETWORK}.config.json"
    try:
        with open(config_path, 'r') as config_file:
            config = json.load(config_file)
            print(f"Loaded configuration from {config_path}")
            return config
    except FileNotFoundError:
        print(f"Error: Configuration file not found at {config_path}")
        exit(1)

async def main():
    print("This process has the PID", os.getpid())

    # Load the config file first
    config = await load_config()

    # Create new SpammerInfo object
    spammer_info: SpammerInfo = SpammerInfo()
    spammer_info.base_url = BASE_URL
    spammer_info.network = NETWORK
    spammer_info.network_number = NETWORK_NUMBER
    spammer_info.config = config

    # Check if we have credentials. If not, generate them. If yes, load them.
    if not os.path.isfile(CREDS_FILENAME):
        await generate_credentials()

    await load_credentials(spammer_info)
    print(f"Your account is {spammer_info.account.as_str()}")

    # Set up spamming config. Values are not validated at the moment.
    spammer_info.spam_message = ""

    # Display loaded config
    print(f"Using LIQUIFY_COMPONENT: {config.get('LIQUIFY_COMPONENT', 'Not found')}")
    print(f"Using LIQUIFY_LIQUIDITY_RECEIPT: {config.get('LIQUIFY_LIQUIDITY_RECEIPT', 'Not found')}")

    # Check what we should do
    choice = int(input("Choose:\n1) Get funds from faucet\n2) Spam liquidity\n3) Spam unstakes\n4) Collect fills\n"))

    # Get funds
    if choice == 1:
        receiving_account = input("\nReceiving account: ")
        start_funds_question = input("\n!! Double-check your input !!\n\nStart getting funds?? y/n: ")
    
        if (start_funds_question.lower() == "y"):
            await start_getting_funds(spammer_info, receiving_account)
        else:
            print("Exiting now.")
            exit()     

    # Spam liquidity
    if choice == 2:
        amount_spammed = int(input("\nAmount spammed already: "))
        
        # New submenu for amount type
        amount_type = int(input("\nChoose amount type:\n1) Random amounts\n2) Set amount\n"))
        
        if amount_type == 2:
            # Set amount option
            set_amount = int(input("\nEnter set amount for each transaction (XRD): "))
            print(f"Will use {set_amount} XRD for each transaction")
        else:
            # Random amount (default)
            set_amount = None
            print("Will use random amounts between 10,000 and 100,000 XRD")
        
        start_spamming_question = input("\n!! Double-check your input !!\n\nStart spamming liquidity? y/n: ")
    
        if (start_spamming_question.lower() == "y"):
            await start_spamming_liquidity(spammer_info, amount_spammed, set_amount)
        else:
            print("Exiting now.")
            exit()  

    # Spam unstakes
    if choice == 3:
        amount_spammed = int(input("\nAmount spammed already: "))
        
        # New submenu for amount type
        amount_type = int(input("\nChoose amount type:\n1) Random amounts\n2) Set amount\n"))
        
        if amount_type == 2:
            # Set amount option
            set_amount = int(input("\nEnter set amount for each transaction (XRD): "))
            print(f"Will use {set_amount} XRD worth of LSUs for each transaction")
        else:
            # Random amount (default)
            set_amount = None
            print("Will use random amounts between 100,000 and 500,000 XRD worth of LSUs")
        
        start_spamming_question = input("\n!! Double-check your input !!\n\nStart spamming unstakes? y/n: ")
    
        if (start_spamming_question.lower() == "y"):
            await start_spamming_unstakes(spammer_info, amount_spammed, set_amount)
        else:
            print("Exiting now.")
            exit()     

    # Collect fills
    if choice == 4:
        start_collecting_question = input("\n!! Double-check your input !!\n\nStart collecting fills? y/n: ")
    
        if (start_collecting_question.lower() == "y"):
            await start_collecting_fills(spammer_info)
        else:
            print("Exiting now.")
            exit()

#
# Get funds from the faucet (10k at a time)
#
async def start_getting_funds(spammer_info: SpammerInfo, account) -> None:
    spammer_info.current_epoch = 0

    print("Getting funds...")

    # Loop
    for i in range(10000):
        # Get current epoch
        spammer_info.current_epoch = await get_current_epoch(spammer_info)

        # build manifest
        manifest = await build_faucet_manifest(spammer_info, account)

        # build tx
        tx = await build_and_sign_transaction(
            manifest,
            spammer_info
        )

        # submit tx
        await submit_transaction(
            tx,
            spammer_info
        )

        print(f"Got funds {i + 1} times")

        await asyncio.sleep(0.5)    

async def start_spamming_liquidity(spammer_info: SpammerInfo, amount_spammed = 0, set_amount = None) -> None:
    spammer_info.current_epoch = await get_current_epoch(spammer_info)

    print(f"Start spamming assuming {amount_spammed} XRD in liquidity already provided...")
    
    # Debug output for the set amount
    if set_amount is not None:
        if set_amount < 10000:
            print(f"WARNING: Set amount ({set_amount} XRD) is below the minimum liquidity requirement of 10,000 XRD.")
            use_anyway = input("Use this amount anyway? This might result in failed transactions. (y/n): ")
            if use_anyway.lower() != 'y':
                print("Please restart the script with a higher amount.")
                exit()
        print(f"Using fixed amount of {set_amount} XRD for each transaction")
    else:
        print("Using random amounts between 10,000 and 100,000 XRD")

    transaction_count = 0
    
    while amount_spammed < 100_000_000:
        # Build manifest
        discount = random.randrange(500, 1_500, 25) # 0.5-1.5% with steps of 0.025%
        
        # Use set_amount if provided, otherwise generate random amount
        if set_amount is not None:
            amount = set_amount
        else:
            amount = random.randrange(10_000, 100_000) # 10k - 100k
            
        auto_unstake = random.choice([True, False])  # Randomly test both modes

        # Get Liquify component and receipt from config
        liquify_component = spammer_info.config.get('LIQUIFY_COMPONENT', '')
        liquidity_receipt = spammer_info.config.get('LIQUIFY_LIQUIDITY_RECEIPT', '')

        # Debug output before building manifest
        print(f"Transaction {transaction_count + 1}: Preparing to add {amount} XRD with {discount/1000:.3f}% discount")
        
        # Create the manifest - Updated with new parameters
        manifest_string = f"""
        CALL_METHOD
            Address("component_tdx_2_1cptxxxxxxxxxfaucetxxxxxxxxx000527798379xxxxxxxxxyulkzl")
            "lock_fee"
            Decimal("100")
        ;
        CALL_METHOD
            Address("{spammer_info.account.as_str()}")
            "withdraw"
            Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
            Decimal("{amount}")
        ;
        TAKE_ALL_FROM_WORKTOP
            Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
            Bucket("xrd_bucket")
        ;
        CALL_METHOD
            Address("{liquify_component}")
            "add_liquidity"
            Bucket("xrd_bucket")
            Decimal("0.{discount:05}")
            {str(auto_unstake).lower()}
            true
            Decimal("10000")
        ;
        TAKE_ALL_FROM_WORKTOP
            Address("{liquidity_receipt}")
            Bucket("receipt_bucket")
        ;
        CALL_METHOD 
            Address("{DEV_ADDRESS}")
            "try_deposit_or_abort"
            Bucket("receipt_bucket")
            Enum<0u8>()
        ;
        CALL_METHOD
            Address("{spammer_info.account.as_str()}")
            "deposit_batch"
            Expression("ENTIRE_WORKTOP")
        ;
        """
        
        # Remove # comments which are not valid in RTM
        manifest_string = "\n".join([line for line in manifest_string.split("\n") if not line.strip().startswith("#")])
        
        try:
            manifest = TransactionManifest(
                Instructions.from_string(manifest_string, spammer_info.network_number),
                []
            )
            manifest.statically_validate()
            
            # Build and sign transaction
            signed_transaction = await build_and_sign_transaction(
                manifest,
                spammer_info
            )

            print(f"{get_timestamp()}: {signed_transaction.intent_hash().as_str()} - Using account {spammer_info.account.as_str()} to provide {amount} XRD of liquidity with a discount of {discount / 1000}%, auto_unstake={auto_unstake}. Receipt will be sent to {DEV_ADDRESS}")
            
            # Submit transaction
            result = await submit_transaction(
                signed_transaction,
                spammer_info
            )
            
            # Debug output for transaction result
            print(f"Transaction result: {result}")

            # Increment the total amount spammed so we eventually reach our stop
            amount_spammed += amount
            transaction_count += 1
            print(f"Total amount spammed: {amount_spammed} XRD after {transaction_count} transactions")

        except Exception as e:
            print(f"Error during transaction: {str(e)}")
            print("Waiting 5 seconds before trying again...")
            await asyncio.sleep(5)
            continue

        await asyncio.sleep(0.5)

    print(f"Done spamming after {transaction_count} transactions")
    exit()

#
# Spam the Liquify component with ~105M XRD in unstakes
#
async def start_spamming_unstakes(spammer_info: SpammerInfo, amount_spammed = 0, set_amount = None) -> None:
    spammer_info.current_epoch = await get_current_epoch(spammer_info)

    print(f"Start spamming assuming {amount_spammed} XRD already unstaked...")

    while amount_spammed < 105_000_000:
        # Build manifest
        # Use set_amount if provided, otherwise generate random amount
        if set_amount is not None:
            amount = set_amount
        else:
            amount = random.randrange(100_000, 500_000) # 100k - 500k

        manifest = await build_unstake_manifest(
            spammer_info,
            amount
        )
        
        # Build and sign transaction
        signed_transaction = await build_and_sign_transaction(
            manifest,
            spammer_info
        )

        print(f"{get_timestamp()}: {signed_transaction.intent_hash().as_str()} - Using account {spammer_info.account.as_str()} to unstake {amount} XRD worth of LSUs.")
        
        # Submit transaction
        await submit_transaction(
            signed_transaction,
            spammer_info
        )

        # Increment the total amount spammed so we eventually reach our stop
        amount_spammed += amount
        print(f"Total amount unstaked: {amount_spammed} XRD worth of LSUs.")

        await asyncio.sleep(0.5)

    print(f"Done spamming")
    exit()    

#
# Collect fills from liquidity receipts
#
async def start_collecting_fills(spammer_info: SpammerInfo) -> None:
    spammer_info.current_epoch = await get_current_epoch(spammer_info)

    print("Collecting fills...")

    # Run for 100 iterations
    for i in range(100):
        # Build manifest
        manifest = await build_collect_fills_manifest(spammer_info)
        
        # Build and sign transaction
        signed_transaction = await build_and_sign_transaction(
            manifest,
            spammer_info
        )

        print(f"{get_timestamp()}: {signed_transaction.intent_hash().as_str()} - Collecting fills")
        
        # Submit transaction
        await submit_transaction(
            signed_transaction,
            spammer_info
        )

        print(f"Collection iteration {i + 1}")

        await asyncio.sleep(0.5)

    print(f"Done collecting fills")

#
# Returns the current epoch
#
async def get_current_epoch(spammer_info: SpammerInfo):
    result = await perform_request(
            spammer_info,
            "/status/gateway-status",
            json.dumps(
                {
                    # "network": spammer_info.network
                }
            )
        )
    
    return result["ledger_state"]["epoch"]

#
# Submits a signed transaction to the network
#
async def submit_transaction(signed_transaction: SignedIntent, spammer_info: SpammerInfo):
    result = await perform_request(
        spammer_info,
        "/transaction/submit",
        json.dumps(
            {
                # "network": spammer_info.network,
                "notarized_transaction_hex": signed_transaction.compile().hex()
                
            }
        )
    )

    return result

#
#  Generically performs a request to the Gateway API endpoint and returns the JSON
#
async def perform_request(spammer_info: SpammerInfo, endpoint, body):
    result = spammer_info.session.post(
            spammer_info.base_url + endpoint, 
            data = body, 
            headers = { 'Content-Type': 'application/json' }
        )
    
    return result.json()

#
# Returns a validated transaction manifest that performs an NFT buy
#
async def build_faucet_manifest(spammer_info: SpammerInfo, account):
    manifest_string: str = f"""
    CALL_METHOD
        Address("component_tdx_2_1cptxxxxxxxxxfaucetxxxxxxxxx000527798379xxxxxxxxxyulkzl")
        "lock_fee"
        Decimal("100")
    ;
    CALL_METHOD
        Address("component_tdx_2_1cptxxxxxxxxxfaucetxxxxxxxxx000527798379xxxxxxxxxyulkzl")
        "free"
    ;
    CALL_METHOD
        Address("{account}")
        "try_deposit_batch_or_abort"
        Expression("ENTIRE_WORKTOP")
        Enum<0u8>()
    ;
    """

    manifest: TransactionManifest = TransactionManifest(
        Instructions.from_string(manifest_string, spammer_info.network_number),
        []
    )
    manifest.statically_validate()

    return manifest 

#
# Returns a validated transaction manifest that adds liquidity to Liquify
#
async def build_liquidity_manifest(spammer_info: SpammerInfo, amount, discount, auto_unstake):
    # Get Liquify component from config
    liquify_component = spammer_info.config.get('LIQUIFY_COMPONENT', '')
    
    manifest_string: str = f"""
    CALL_METHOD
        Address("component_tdx_2_1cptxxxxxxxxxfaucetxxxxxxxxx000527798379xxxxxxxxxyulkzl")
        "lock_fee"
        Decimal("100")
    ;
    CALL_METHOD
        Address("{spammer_info.account.as_str()}")
        "withdraw"
        Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
        Decimal("{amount}")
    ;
    TAKE_ALL_FROM_WORKTOP
        Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
        Bucket("xrd_bucket")
    ;
    CALL_METHOD
        Address("{liquify_component}")
        "add_liquidity"
        Bucket("xrd_bucket")
        Decimal("0.{discount:05}")
        {str(auto_unstake).lower()}
        true
        Decimal("10000")
    ;
    CALL_METHOD
        Address("{spammer_info.account.as_str()}")
        "deposit_batch"
        Expression("ENTIRE_WORKTOP")
    ;
    """

    manifest: TransactionManifest = TransactionManifest(
        Instructions.from_string(manifest_string, spammer_info.network_number),
        []
    )
    manifest.statically_validate()

    return manifest

#
# Returns a validated transaction manifest that unstakes via Liquify
#
async def build_unstake_manifest(spammer_info: SpammerInfo, amount):
    # Get Liquify component from config
    liquify_component = spammer_info.config.get('LIQUIFY_COMPONENT', '')
    
    # Hardcode max_iterations to 26
    max_iterations = 26
    
    manifest_string: str = f"""
    CALL_METHOD
        Address("component_tdx_2_1cptxxxxxxxxxfaucetxxxxxxxxx000527798379xxxxxxxxxyulkzl")
        "lock_fee"
        Decimal("100")
    ;    
    CALL_METHOD
        Address("{spammer_info.account.as_str()}")
        "withdraw"
        Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
        Decimal("{amount}")
    ;
    TAKE_ALL_FROM_WORKTOP
        Address("resource_tdx_2_1tknxxxxxxxxxradxrdxxxxxxxxx009923554798xxxxxxxxxtfd2jc")
        Bucket("xrd_bucket")
    ;
    CALL_METHOD
        Address("validator_tdx_2_1sdlkptcwjpajqawnuya8r2mgl3eqt89hw27ww6du8kxmx3thmyu8l4")
        "stake"
        Bucket("xrd_bucket")
    ;
    TAKE_ALL_FROM_WORKTOP
        Address("resource_tdx_2_1t5hpjckz9tm63gqvxsl60ejhzvnlguly77tltvywnj06s2x9wjdxjn")
        Bucket("lsu_bucket")
    ;
    CALL_METHOD
        Address("{liquify_component}")
        "liquify_unstake"
        Bucket("lsu_bucket")
        {max_iterations}u8
    ;
    CALL_METHOD
        Address("{spammer_info.account.as_str()}")
        "deposit_batch"
        Expression("ENTIRE_WORKTOP")
    ;
    """

    manifest: TransactionManifest = TransactionManifest(
        Instructions.from_string(manifest_string, spammer_info.network_number),
        []
    )
    manifest.statically_validate()

    return manifest

#
# Returns a validated transaction manifest that collects fills from liquidity receipts
#
async def build_collect_fills_manifest(spammer_info: SpammerInfo):
    # Get Liquify component and receipt from config
    liquify_component = spammer_info.config.get('LIQUIFY_COMPONENT', '')
    liquidity_receipt = spammer_info.config.get('LIQUIFY_LIQUIDITY_RECEIPT', '')
    
    # Hardcode number of fills to 50
    number_of_fills = 50
    
    manifest_string: str = f"""
    CALL_METHOD
        Address("component_tdx_2_1cptxxxxxxxxxfaucetxxxxxxxxx000527798379xxxxxxxxxyulkzl")
        "lock_fee"
        Decimal("100")
    ;
    CALL_METHOD
        Address("{spammer_info.account.as_str()}")
        "withdraw"
        Address("{liquidity_receipt}")
        Decimal("1")
    ;
    TAKE_ALL_FROM_WORKTOP
        Address("{liquidity_receipt}")
        Bucket("receipt_bucket")
    ;
    CALL_METHOD
        Address("{liquify_component}")
        "collect_fills"
        Bucket("receipt_bucket")
        {number_of_fills}u64
    ;
    CALL_METHOD
        Address("{spammer_info.account.as_str()}")
        "deposit_batch"
        Expression("ENTIRE_WORKTOP")
    ;
    """

    manifest: TransactionManifest = TransactionManifest(
        Instructions.from_string(manifest_string, spammer_info.network_number),
        []
    )
    manifest.statically_validate()

    return manifest

#
#  Builds and signs a transaction
#
async def build_and_sign_transaction(transaction_manifest: TransactionManifest, spammer_info: SpammerInfo):
    transaction: NotarizedTransaction = (
        TransactionBuilder()
        .header(
            TransactionHeader(
                spammer_info.network_number,
                spammer_info.current_epoch,
                spammer_info.current_epoch + 1000,
                random_nonce(),
                spammer_info.public_key,
                True,
                0,
            )
        )
        .manifest(transaction_manifest)
        .message(
            Message.PLAIN_TEXT(
                PlainTextMessage("text/plain", MessageContent.STR(spammer_info.spam_message))
            )
        )
        .notarize_with_private_key(spammer_info.private_key)
    )

    return transaction 

#
# Generates a random nonce to make transactions unique
#
def random_nonce() -> int:
    return secrets.randbelow(0xFFFFFFFF)

#
# Generate credentials and write to disk
#
async def generate_credentials():
    private_key_bytes: bytes = secrets.randbits(256).to_bytes(32, sys.byteorder)
    private_key_hex: str = private_key_bytes.hex()
    private_key: PrivateKey = PrivateKey.new_secp256k1(private_key_bytes)

    (_public_key, account) = await derive_account(private_key)

    with open(CREDS_FILENAME, "w") as file:
        json.dump(
            {
                "private_key": private_key_hex,
                "account": account.as_str()
            },
            file
        )

#
# Derive an account
#
async def derive_account(private_key):
    public_key: PublicKey = private_key.public_key()
    account: Address = derive_virtual_account_address_from_public_key(
        public_key, NETWORK_NUMBER
    )

    return (public_key, account)

#
# Read credentials from disk
#
async def load_credentials(spammer_info: SpammerInfo):   
    with open(CREDS_FILENAME, "r") as file:
        content: dict[str, str] = json.load(file)

    private_key_hex: str = content["private_key"]
    private_key_bytes: bytearray = bytearray.fromhex(private_key_hex)

    private_key = PrivateKey.new_secp256k1(private_key_bytes)
    (public_key, account) = await derive_account(private_key)

    spammer_info.private_key = private_key
    spammer_info.public_key = public_key
    spammer_info.account = account

# Generates a timestamp string
def get_timestamp():
    return datetime.now().strftime("%m/%d/%Y, %H:%M:%S.%f")

if __name__ == '__main__':
    asyncio.run(main())