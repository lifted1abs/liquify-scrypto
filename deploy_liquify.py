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

# Network Configuration
NETWORK = "stokenet"  # "stokenet" or "mainnet"
NETWORK_ID = 2 if NETWORK == "stokenet" else 1
DOCKER_IMAGE = "radixdlt/scrypto-builder:v1.2.0"
DOCKER_PLATFORM = "linux/amd64"

# Your personal address where you want owner badges sent
PERSONAL_ADDRESS = "account_tdx_2_1298qr4yymzfvjfqn48f5k00r79snw695zln0lxele0c2jgrwsdhwkc"

# Gateway URL
GATEWAY_URL = os.getenv('GATEWAY_URL')
if not GATEWAY_URL:
    if NETWORK == "stokenet":
        GATEWAY_URL = "https://stokenet.radixdlt.com"
    else:
        GATEWAY_URL = "https://mainnet.radixdlt.com"
    print(f"Using default gateway URL: {GATEWAY_URL}")

# ===== END CONFIGURATION =====

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
    print("Building project with Docker...")
    
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

def read_hex(file_path):
    """Read a file and return its hex representation."""
    with open(file_path, "rb") as f:
        return f.read().hex()

async def main():
    try:
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
        os.environ['GATEWAY_URL'] = GATEWAY_URL

        async with ClientSession(connector=TCPConnector(ssl=False)) as session:
            gateway = Gateway(session)
            network_config = await gateway.network_configuration()
            
            # Check if we're on the correct network
            if network_config['network_name'] != NETWORK:
                print(f"Error: Expected {NETWORK} but connected to {network_config['network_name']}")
                exit(1)
            
            # Load or create account
            account_details = load_account(network_config['network_id'])
            if account_details is None:
                account_details = new_account(network_config['network_id'])
            private_key, public_key, account = account_details

            print(f"Deploying with account: {account.as_str()}")
            print(f"Network: {network_config['network_name']}")

            # Check and get funds if needed
            balance = 0
            try:
                balance = await gateway.get_xrd_balance(account)
            except Exception as e:
                print(f"Note: Account not yet on network (expected for new accounts): {e}")
            
            print(f"Current balance: {balance} XRD")
            
            # Get funds from faucet if balance is low and we're on Stokenet
            if balance < 500 and NETWORK == "stokenet":
                balance = await get_funds_from_faucet(gateway, account, public_key, private_key)
            
            # Check if we have sufficient funds
            if balance < 100:
                print('\n=== FUND ACCOUNT ===')
                print(f'Address: {account.as_str()}')
                print('Minimum 100 XRD required')
                
                if network_config['network_name'] == 'stokenet':
                    print('Attempting to get more funds from faucet...')
                    # Try faucet 3 more times
                    for i in range(3):
                        if balance >= 100:
                            break
                        print(f"Faucet attempt {i+2}...")
                        balance = await get_funds_from_faucet(gateway, account, public_key, private_key)
                        await asyncio.sleep(2)
                    
                    if balance < 100:
                        print('Please fund manually using faucet or run: python liquify_spammer.py')
                else:
                    print('Please send XRD to this address')
                
                while balance < 100:
                    await asyncio.sleep(5)
                    balance = await gateway.get_xrd_balance(account)

            # Load config or create new one
            config_path = script_path / f"{NETWORK}.config.json"
            print(f"Config path: {config_path}")
            try:
                with open(config_path, 'r') as config_file:
                    config_data = json.load(config_file)
                print("Config loaded successfully")
            except FileNotFoundError:
                print("Creating new config file")
                config_data = {}

            # Build with Docker
            print("\n=== Building Liquify Package ===")
            
            # Temporarily change back to scrypto directory for building
            os.chdir(scrypto_path)
            build_with_docker()
            
            # Change back to script directory
            os.chdir(script_path)

            # Find WASM and RPD files - use Scrypto project path
            output_path = scrypto_path / "target" / "wasm32-unknown-unknown" / "release"
            wasm_file = output_path / f"{PACKAGE_NAME}.wasm"
            rpd_file = output_path / f"{PACKAGE_NAME}.rpd"
            
            print(f"Looking for build artifacts at: {output_path}")
            print(f"Checking for WASM file: {wasm_file}")
            print(f"Checking for RPD file: {rpd_file}")
            
            if not wasm_file.exists() or not rpd_file.exists():
                print(f"Error: Build artifacts not found")
                print("Let's check what's in the output directory:")
                if output_path.exists():
                    for file in output_path.glob("*"):
                        print(f"  {file.name}")
                else:
                    print(f"  Directory doesn't exist: {output_path}")
                exit(1)

            print(f"Found WASM file: {wasm_file}")
            print(f"Found RPD file: {rpd_file}")

            # Deploy package if not already deployed
            if 'LIQUIFY_PACKAGE' not in config_data:
                print("\n=== Deploying Liquify Package ===")
                
                # Use exactly the same pattern as Surge deployment
                import radix_engine_toolkit as ret
                
                # Create owner role for the package - using the resource address
                owner_amount = '1'
                owner_resource = 'resource_tdx_2_1t40nwpv3ra5k34wy2nug6pds2mup7gszxwkgsg68mcvx550q6zmpkw'  # Hardcoded as requested
                owner_role = ret.OwnerRole.UPDATABLE(ret.AccessRule.require_amount(ret.Decimal(owner_amount), ret.Address(owner_resource)))
                
                # Read the files as Surge does
                with open(wasm_file, 'rb') as f:
                    code = f.read()
                with open(rpd_file, 'rb') as f:
                    definition = f.read()
                
                # Use gateway.build_publish_transaction exactly like Surge
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

            # Set platform fee and enable the component BEFORE transferring badges
            if 'LIQUIFY_COMPONENT' in config_data:
                # First set platform fee
                print("\n=== Setting Platform Fee ===")
                
                platform_fee_input = input("Enter platform fee percentage (e.g., 1 for 1%, 0.01 for 0.01%): ")
                try:
                    platform_fee = float(platform_fee_input) / 100  # Convert percentage to decimal
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
                    
                    print("Setting platform fee...")
                    payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                    await gateway.submit_transaction(payload)
                    status = await gateway.get_transaction_status(intent)
                    
                    if status == "CommittedSuccess":
                        print(f"✓ Platform fee set to {platform_fee_input}% successfully!")
                    else:
                        print(f"✗ Failed to set platform fee: {status}")
                
                except ValueError:
                    print("Invalid input for platform fee. Skipping fee configuration.")
                
                # Now enable the component
                print("\n=== Enabling Liquify Component ===")
                
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
                
                print("Enabling component...")
                payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                await gateway.submit_transaction(payload)
                status = await gateway.get_transaction_status(intent)
                
                if status == "CommittedSuccess":
                    print("✓ Component enabled successfully!")
                else:
                    print(f"✗ Failed to enable component: {status}")

            # Transfer owner badges to your personal address AFTER enabling
            if 'LIQUIFY_OWNER_BADGE' in config_data and PERSONAL_ADDRESS:
                print("\n=== Transferring Owner Badges ===")
                
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
                    Address("{PERSONAL_ADDRESS}")
                    "try_deposit_batch_or_abort"
                    Expression("ENTIRE_WORKTOP")
                    Enum<0u8>()
                ;
                """
                
                print(f"Transferring owner badge to: {PERSONAL_ADDRESS}")
                payload, intent = await gateway.build_transaction_str(manifest, public_key, private_key)
                await gateway.submit_transaction(payload)
                status = await gateway.get_transaction_status(intent)
                
                if status == "CommittedSuccess":
                    print("✓ Owner badges successfully transferred!")
                else:
                    print(f"✗ Transfer failed with status: {status}")

            # Save updated config
            print(f"\nSaving config to: {config_path}")
            with open(config_path, 'w') as config_file:
                json.dump(config_data, config_file, indent=4)
            print(f"✓ Configuration saved to {config_path}")

            print("\n=== Deployment Complete ===")
            print(f"Package address: {config_data.get('LIQUIFY_PACKAGE', 'N/A')}")
            print(f"Component address: {config_data.get('LIQUIFY_COMPONENT', 'N/A')}")
            print(f"Owner badge: {config_data.get('LIQUIFY_OWNER_BADGE', 'N/A')}")
            print(f"Liquidity receipt: {config_data.get('LIQUIFY_LIQUIDITY_RECEIPT', 'N/A')}")

    except Exception as e:
        print(f"\nError during deployment: {e}")
        print("Traceback:")
        print(traceback.format_exc())

if __name__ == "__main__":
    asyncio.run(main())