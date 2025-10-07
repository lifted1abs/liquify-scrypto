#!/usr/bin/env python3
import os
import subprocess
import json
import asyncio
import sys
import traceback
from pathlib import Path
from aiohttp import ClientSession, TCPConnector
from subprocess import run
from dotenv import load_dotenv

# Add the script directory to Python path
script_dir = Path(__file__).parent
sys.path.append(str(script_dir))

# Load environment variables
load_dotenv(script_dir / '.env')

from tools.gateway import Gateway
from tools.accounts import new_account, load_account  
from tools.manifests import lock_fee, deposit_all

# ===== DEPLOYMENT CONFIGURATION =====
# Package Configuration
PACKAGE_NAME = "liquify_scrypto"
BLUEPRINT_NAME = "Liquify"
BLUEPRINT_FUNCTION = "instantiate_liquify"
INTERFACE_BLUEPRINT_NAME = "LiquifyInterface"
INTERFACE_FUNCTION = "instantiate_interface"

# Docker Configuration
DOCKER_IMAGE = "radixdlt/scrypto-builder:v1.2.0"
DOCKER_PLATFORM = "linux/amd64"

# Network configurations
NETWORK_CONFIGS = {
    "mainnet": {
        "network_id": 1,
        "gateway_url": "https://mainnet.radixdlt.com",
        "min_balance": 200,
        "package_owner_badge": "resource_rdx1t529jw5nfhyf3ujrznmfqwvcj4gs30lpcgqcy5c3tvp0e6p503vlxc"
    },
    "stokenet": {
        "network_id": 2,
        "gateway_url": "https://stokenet.radixdlt.com",
        "min_balance": 100,
        "package_owner_badge": "resource_tdx_2_1t40nwpv3ra5k34wy2nug6pds2mup7gszxwkgsg68mcvx550q6zmpkw"
    }
}

# ===== END CONFIGURATION =====

def ask_yes_no(question):
    """Ask a yes/no question and return boolean."""
    while True:
        answer = input(f"{question} (y/n): ").lower().strip()
        if answer in ['y', 'yes']:
            return True
        elif answer in ['n', 'no']:
            return False
        else:
            print("Please answer 'y' or 'n'")

def choose_network():
    """Let user choose between mainnet and stokenet using numbers."""
    print("\n=== Network Selection ===")
    print("1) Mainnet (production)")
    print("2) Stokenet (testnet)")
    
    while True:
        choice = input("\nSelect network (1 or 2): ").strip()
        if choice == "1":
            return "mainnet"
        elif choice == "2":
            return "stokenet"
        else:
            print("Please enter 1 or 2")

async def get_funds_from_faucet(gateway, account, public_key, private_key):
    """Get test XRD from the Stokenet faucet."""
    print("\n=== Getting XRD from faucet ===")
    
    manifest = f"""
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
        Address("{account.as_str()}")
        "try_deposit_batch_or_abort"
        Expression("ENTIRE_WORKTOP")
        Enum<0u8>()
    ;
    """
    
    try:
        payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
        await gateway.submit_transaction(payload)
        status = await gateway.get_transaction_status(intent)
        print(f"Faucet transaction status: {status}")
        
        # Wait for transaction to be processed
        await asyncio.sleep(2)
        
        # Check new balance
        balance = await gateway.get_xrd_balance(account)
        print(f"New balance: {balance} XRD")
        return balance
    except Exception as e:
        print(f"Failed to get funds from faucet: {e}")
        return 0

def build_with_docker():
    """Build the project using Docker."""
    print("\n=== Building project with Docker ===")
    
    # Use current directory (should be the scrypto project)
    project_path = Path.cwd()
    cargo_file = project_path / "Cargo.toml"
    
    if not cargo_file.exists():
        print(f"Error: Cargo.toml not found at {project_path}")
        print("Make sure you're running this script from the Scrypto project directory")
        exit(1)
    
    # Get absolute path for volume mount
    project_abs_path = str(project_path.absolute())
    print(f"Building project at: {project_abs_path}")
    
    env = os.environ.copy()
    env["DOCKER_DEFAULT_PLATFORM"] = DOCKER_PLATFORM
    
    try:
        # Pull Docker image
        subprocess.run(
            ["docker", "pull", DOCKER_IMAGE],
            env=env,
            check=True
        )
        
        # Run Docker build
        result = subprocess.run(
            ["docker", "run", "--rm", "-v", f"{project_abs_path}:/src", DOCKER_IMAGE],
            env=env,
            capture_output=True,
            text=True
        )
        
        print("Docker build stdout:", result.stdout)
        
        if result.returncode != 0:
            print(f"Error building with Docker: {result.stderr}")
            exit(1)
        
        print("✓ Docker build completed successfully")
        
    except subprocess.CalledProcessError as e:
        print(f"Error running Docker build: {e}")
        exit(1)

async def main():
    try:
        # Choose network first - using numbers as requested
        network = choose_network()
        network_config = NETWORK_CONFIGS[network]
        
        print(f"\n=== Deploying to {network.upper()} ===")
        print(f"Network ID: {network_config['network_id']}")
        print(f"Gateway URL: {network_config['gateway_url']}")
        print(f"Minimum balance required: {network_config['min_balance']} XRD")
        print(f"Package owner badge: {network_config['package_owner_badge']}")
        
        # Save the current working directory (should be the scrypto project)
        scrypto_path = Path.cwd()
        
        # Check if we're in the right directory
        if not (scrypto_path / "Cargo.toml").exists():
            print("Error: Must run this script from the Scrypto project directory")
            print(f"Current directory: {scrypto_path}")
            exit(1)
        
        # Change to the script directory for config files
        script_path = Path(__file__).parent
        os.chdir(script_path)
        print(f"Script directory: {script_path}")

        # Set environment variable for Gateway
        os.environ['GATEWAY_URL'] = network_config['gateway_url']

        async with ClientSession(connector=TCPConnector(ssl=False)) as session:
            gateway = Gateway(session)
            gateway_network_config = await gateway.network_configuration()
            
            # Check if we're on the correct network
            if gateway_network_config['network_name'] != network:
                print(f"Error: Expected {network} but connected to {gateway_network_config['network_name']}")
                exit(1)
            
            # Load or create account
            account_details = load_account(gateway_network_config['network_id'])
            if account_details is None:
                account_details = new_account(gateway_network_config['network_id'])
            private_key, public_key, account = account_details

            print(f"\nDeploying with account: {account.as_str()}")
            print(f"Network: {gateway_network_config['network_name']}")

            # Check balance
            balance = 0
            try:
                balance = await gateway.get_xrd_balance(account)
            except Exception as e:
                # This is expected for new accounts that haven't been used yet
                print(f"Note: Account not yet on network (expected for new accounts)")
                balance = 0
            
            print(f"Current balance: {balance} XRD")
            
            # Handle insufficient funds differently for mainnet vs stokenet
            min_balance = network_config['min_balance']
            
            if balance < min_balance:
                if network == "stokenet":
                    print(f"\n=== Insufficient balance (need {min_balance} XRD) ===")
                    print("Attempting to get funds from faucet...")
                    
                    # Try faucet up to 3 times
                    for i in range(3):
                        balance = await get_funds_from_faucet(gateway, account, public_key, private_key)
                        if balance >= min_balance:
                            break
                        if i < 2:
                            print(f"Still insufficient ({balance} XRD). Trying again...")
                            await asyncio.sleep(2)
                    
                    if balance < min_balance:
                        print(f"\nFaucet didn't provide enough funds. Current balance: {balance} XRD")
                        print(f"Please manually fund the account or run: python liquify_spammer.py")
                else:
                    # Mainnet - user must fund manually
                    print(f"\n=== INSUFFICIENT FUNDS ===")
                    print(f"Transaction failed due to insufficient XRD")
                    print(f"Please send at least {min_balance} XRD to the following address:")
                    print(f"\n{account.as_str()}\n")
                    print(f"Current balance: {balance} XRD")
                    print(f"Required: {min_balance} XRD")
                
                # Wait for funds for both networks
                while balance < min_balance:
                    await asyncio.sleep(5)
                    try:
                        balance = await gateway.get_xrd_balance(account)
                        if int(balance) % 10 == 0 and balance > 0:  # Print every 10 XRD increment
                            print(f"Current balance: {balance} XRD")
                    except:
                        pass  # Account might not exist yet

            print(f"✓ Sufficient balance: {balance} XRD")

            # Load config or create new one
            config_path = script_path / f"{network}.config.json"
            print(f"\nConfig path: {config_path}")
            try:
                with open(config_path, 'r') as config_file:
                    config_data = json.load(config_file)
                print("Config loaded successfully")
            except FileNotFoundError:
                print("Creating new config file")
                config_data = {}

            # Store package owner badge in config for reference
            config_data['PACKAGE_OWNER_BADGE'] = network_config['package_owner_badge']

            # Build with Docker
            print("\n=== Building Liquify Package ===")
            
            # Temporarily change back to scrypto directory for building
            os.chdir(scrypto_path)
            build_with_docker()
            
            # Change back to script directory
            os.chdir(script_path)

            # Find WASM and RPD files
            output_path = scrypto_path / "target" / "wasm32-unknown-unknown" / "release"
            wasm_file = output_path / f"{PACKAGE_NAME}.wasm"
            rpd_file = output_path / f"{PACKAGE_NAME}.rpd"
            
            if not wasm_file.exists() or not rpd_file.exists():
                print(f"Error: Build artifacts not found")
                exit(1)

            # Deploy package if not already deployed
            if 'LIQUIFY_PACKAGE' not in config_data:
                print("\n=== Deploying Liquify Package ===")
                
                import radix_engine_toolkit as ret
                
                # Create owner role for the package
                owner_amount = '1'
                owner_resource = network_config['package_owner_badge']
                owner_role = ret.OwnerRole.UPDATABLE(ret.AccessRule.require_amount(ret.Decimal(owner_amount), ret.Address(owner_resource)))
                
                # Read the files
                with open(wasm_file, 'rb') as f:
                    code = f.read()
                with open(rpd_file, 'rb') as f:
                    definition = f.read()
                
                # Deploy package
                payload, intent = await gateway.build_publish_transaction(
                    account,
                    code,
                    definition,
                    owner_role,
                    public_key,
                    private_key,
                )
                
                await gateway.submit_transaction(payload)
                addresses = await gateway.get_new_addresses(intent)
                config_data['LIQUIFY_PACKAGE'] = addresses[0]
                print(f"LIQUIFY_PACKAGE: {addresses[0]}")

            # Deploy component if not already deployed
            if 'LIQUIFY_COMPONENT' not in config_data:
                print("\n=== Instantiating Liquify Component ===")
                
                manifest = f"""
                CALL_METHOD
                    Address("{account.as_str()}")
                    "lock_fee"
                    Decimal("100")
                ;
                CALL_FUNCTION
                    Address("{config_data['LIQUIFY_PACKAGE']}")
                    "{BLUEPRINT_NAME}"
                    "{BLUEPRINT_FUNCTION}"
                    ;
                CALL_METHOD
                    Address("{account.as_str()}")
                    "deposit_batch"
                    Expression("ENTIRE_WORKTOP")
                ;
                """
                
                print("Submitting component instantiation transaction...")
                payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                await gateway.submit_transaction(payload)
                print(f"Waiting for transaction: {intent}")
                addresses = await gateway.get_new_addresses(intent)
                
                component_address = addresses[0]
                config_data['LIQUIFY_COMPONENT'] = component_address
                config_data['LIQUIFY_OWNER_BADGE'] = addresses[1]
                config_data['LIQUIFY_LIQUIDITY_RECEIPT'] = addresses[2]
                
                print(f"LIQUIFY_COMPONENT: {component_address}")
                print(f"LIQUIFY_OWNER_BADGE: {addresses[1]}")
                print(f"LIQUIFY_LIQUIDITY_RECEIPT: {addresses[2]}")

            # Configure component if needed
            if 'LIQUIFY_COMPONENT' in config_data:
                # Set platform fee - ask regardless of network
                print("\n=== Platform Fee Configuration ===")
                platform_fee_input = input("Enter platform fee percentage (e.g., 1 for 1%, 0.01 for 0.01%): ")
                try:
                    platform_fee = float(platform_fee_input) / 100
                    print(f"Setting platform fee to {platform_fee_input}% ({platform_fee} as decimal)")
                    
                    manifest = f"""
                    CALL_METHOD
                        Address("{account.as_str()}")
                        "lock_fee"
                        Decimal("10")
                    ;
                    CALL_METHOD
                        Address("{account.as_str()}")
                        "create_proof_of_amount"
                        Address("{config_data['LIQUIFY_OWNER_BADGE']}")
                        Decimal("1")
                    ;
                    CALL_METHOD
                        Address("{config_data['LIQUIFY_COMPONENT']}")
                        "set_platform_fee"
                        Decimal("{platform_fee}")
                    ;
                    CALL_METHOD
                        Address("{account.as_str()}")
                        "deposit_batch"
                        Expression("ENTIRE_WORKTOP")
                    ;
                    """
                    
                    payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                    await gateway.submit_transaction(payload)
                    status = await gateway.get_transaction_status(intent)
                    
                    if status == "CommittedSuccess":
                        print(f"✓ Platform fee set to {platform_fee_input}% successfully!")
                    else:
                        print(f"✗ Failed to set platform fee: {status}")
                
                except ValueError:
                    print("Invalid input for platform fee. Skipping.")
                
                # Enable component - ask regardless of network
                print("\n=== Component Activation ===")
                if ask_yes_no("Would you like to enable the component?"):
                    manifest = f"""
                    CALL_METHOD
                        Address("{account.as_str()}")
                        "lock_fee"
                        Decimal("10")
                    ;
                    CALL_METHOD
                        Address("{account.as_str()}")
                        "create_proof_of_amount"
                        Address("{config_data['LIQUIFY_OWNER_BADGE']}")
                        Decimal("1")
                    ;
                    CALL_METHOD
                        Address("{config_data['LIQUIFY_COMPONENT']}")
                        "set_component_status"
                        true
                    ;
                    CALL_METHOD
                        Address("{account.as_str()}")
                        "deposit_batch"
                        Expression("ENTIRE_WORKTOP")
                    ;
                    """
                    
                    payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                    await gateway.submit_transaction(payload)
                    status = await gateway.get_transaction_status(intent)
                    
                    if status == "CommittedSuccess":
                        print("✓ Component enabled successfully!")
                    else:
                        print(f"✗ Failed to enable component: {status}")

# Deploy interface component if not already deployed
            if 'LIQUIFY_INTERFACE_COMPONENT' not in config_data:
                print("\n=== Instantiating Liquify Interface Component ===")
                
                manifest = f"""
                CALL_METHOD
                    Address("{account.as_str()}")
                    "lock_fee"
                    Decimal("100")
                ;
                CALL_FUNCTION
                    Address("{config_data['LIQUIFY_PACKAGE']}")
                    "{INTERFACE_BLUEPRINT_NAME}"
                    "{INTERFACE_FUNCTION}"
                    ;
                CALL_METHOD
                    Address("{account.as_str()}")
                    "deposit_batch"
                    Expression("ENTIRE_WORKTOP")
                ;
                """
                
                print("Submitting interface instantiation transaction...")
                payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                await gateway.submit_transaction(payload)
                print(f"Waiting for transaction: {intent}")
                addresses = await gateway.get_new_addresses(intent)
                
                print(f"\n=== DEBUG: All addresses returned ===")
                for i, addr in enumerate(addresses):
                    print(f"addresses[{i}]: {addr}")
                
                interface_component = addresses[0]
                interface_owner_badge_resource = addresses[1]
                config_data['LIQUIFY_INTERFACE_COMPONENT'] = interface_component
                config_data['LIQUIFY_INTERFACE_OWNER_BADGE'] = interface_owner_badge_resource
                
                print(f"LIQUIFY_INTERFACE_COMPONENT: {interface_component}")
                print(f"LIQUIFY_INTERFACE_OWNER_BADGE: {interface_owner_badge_resource}")

            # Set interface target automatically
            if 'LIQUIFY_INTERFACE_COMPONENT' in config_data and 'LIQUIFY_COMPONENT' in config_data:
                print("\n=== Setting Interface Target ===")
                print(f"Pointing interface to Liquify component: {config_data['LIQUIFY_COMPONENT']}")
                
                manifest = f"""
                CALL_METHOD
                    Address("{account.as_str()}")
                    "lock_fee"
                    Decimal("10")
                ;
                CALL_METHOD
                    Address("{account.as_str()}")
                    "create_proof_of_amount"
                    Address("{config_data['LIQUIFY_INTERFACE_OWNER_BADGE']}")
                    Decimal("1")
                ;
                CALL_METHOD
                    Address("{config_data['LIQUIFY_INTERFACE_COMPONENT']}")
                    "set_interface_target"
                    Address("{config_data['LIQUIFY_COMPONENT']}")
                ;
                CALL_METHOD
                    Address("{account.as_str()}")
                    "deposit_batch"
                    Expression("ENTIRE_WORKTOP")
                ;
                """
                
                payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                await gateway.submit_transaction(payload)
                status = await gateway.get_transaction_status(intent)
                
                if status == "CommittedSuccess":
                    print("✓ Interface target set successfully!")
                else:
                    print(f"✗ Failed to set interface target: {status}")

            # Transfer both owner badges if desired
            if 'LIQUIFY_OWNER_BADGE' in config_data and 'LIQUIFY_INTERFACE_OWNER_BADGE' in config_data:
                print("\n=== Owner Badge Transfer ===")
                if ask_yes_no("Would you like to transfer both owner badges to another account?"):
                    personal_address = input("Enter the destination account address: ").strip()
                    
                    # Validate address format
                    if personal_address.startswith("account_"):
                        manifest = f"""
                        CALL_METHOD
                            Address("{account.as_str()}")
                            "lock_fee"
                            Decimal("10")
                        ;
                        CALL_METHOD
                            Address("{account.as_str()}")
                            "withdraw"
                            Address("{config_data['LIQUIFY_OWNER_BADGE']}")
                            Decimal("1")
                        ;
                        CALL_METHOD
                            Address("{account.as_str()}")
                            "withdraw"
                            Address("{config_data['LIQUIFY_INTERFACE_OWNER_BADGE']}")
                            Decimal("1")
                        ;
                        CALL_METHOD
                            Address("{personal_address}")
                            "try_deposit_batch_or_abort"
                            Expression("ENTIRE_WORKTOP")
                            Enum<0u8>()
                        ;
                        """
                        
                        print(f"Transferring both owner badges to: {personal_address}")
                        payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                        await gateway.submit_transaction(payload)
                        status = await gateway.get_transaction_status(intent)
                        
                        if status == "CommittedSuccess":
                            print("✓ Both owner badges successfully transferred!")
                        else:
                            print(f"✗ Transfer failed with status: {status}")
                    else:
                        print("Invalid address format. Skipping transfer.")

            # Save updated config
            print(f"\nSaving config to: {config_path}")
            with open(config_path, 'w') as config_file:
                json.dump(config_data, config_file, indent=4)
            print(f"✓ Configuration saved to {config_path}")

            print("\n=== Deployment Complete ===")
            print(f"Network: {network}")
            print(f"Package address: {config_data.get('LIQUIFY_PACKAGE', 'N/A')}")
            print(f"Package owner badge: {config_data.get('PACKAGE_OWNER_BADGE', 'N/A')}")
            print(f"Component address: {config_data.get('LIQUIFY_COMPONENT', 'N/A')}")
            print(f"Component owner badge: {config_data.get('LIQUIFY_OWNER_BADGE', 'N/A')}")
            print(f"Liquidity receipt: {config_data.get('LIQUIFY_LIQUIDITY_RECEIPT', 'N/A')}")
            print(f"Interface component: {config_data.get('LIQUIFY_INTERFACE_COMPONENT', 'N/A')}")
            print(f"Interface owner badge: {config_data.get('LIQUIFY_INTERFACE_OWNER_BADGE', 'N/A')}")

    except Exception as e:
        print(f"\nError during deployment: {e}")
        print("Traceback:")
        print(traceback.format_exc())

if __name__ == "__main__":
    asyncio.run(main())