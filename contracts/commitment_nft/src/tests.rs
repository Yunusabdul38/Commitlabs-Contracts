#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Events, Ledger}, Address, Env, String};

// Test helpers and fixtures
pub struct TestFixture {
    pub env: Env,
    pub contract_id: Address,
    pub client: CommitmentNFTContractClient<'static>,
    pub admin: Address,
    pub owner: Address,
    pub user1: Address,
    pub user2: Address,
}

impl TestFixture {
    pub fn setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CommitmentNFTContract);
        let client = CommitmentNFTContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);

        // Initialize contract
        client.initialize(&admin).unwrap();

        TestFixture {
            env,
            contract_id,
            client,
            admin,
            owner,
            user1,
            user2,
        }
    }

    pub fn create_test_metadata(&self) -> (String, u32, u32, String, i128, Address) {
        (
            String::from_str(&self.env, "test_commitment_1"),
            30,
            10,
            String::from_str(&self.env, "safe"),
            1000_0000000,
            Address::generate(&self.env),
        )
    }
}

// Unit Tests for Commitment NFT Contract

#[test]
fn test_initialize() {
    let (e, contract_id, admin) = {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, CommitmentNFTContract);
        let admin = Address::generate(&e);
        (e, contract_id, admin)
    };
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    let result = client.initialize(&admin);
    assert_eq!(result, Ok(()));

    // Verify total supply is 0
    let supply = client.total_supply().unwrap();
    assert_eq!(supply, 0);
}

#[test]
fn test_initialize_twice_fails() {
    let (e, contract_id, admin) = {
        let e = Env::default();
        e.mock_all_auths();
        let contract_id = e.register_contract(None, CommitmentNFTContract);
        let admin = Address::generate(&e);
        (e, contract_id, admin)
    };
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin).unwrap();
    let result = client.try_initialize(&admin);
    assert!(result.is_err());
}

#[test]
fn test_mint_success() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    assert_eq!(token_id, 1);

    // Verify metadata
    let metadata = fixture.client.get_metadata(&token_id).unwrap();
    assert_eq!(metadata.commitment_id, commitment_id);
    assert_eq!(metadata.duration_days, duration);
    assert_eq!(metadata.max_loss_percent, max_loss);
    assert_eq!(metadata.commitment_type, c_type);
    assert_eq!(metadata.initial_amount, amount);
    assert_eq!(metadata.asset_address, asset);

    // Verify owner
    let owner = fixture.client.owner_of(&token_id).unwrap();
    assert_eq!(owner, fixture.owner);

    // Verify active status
    assert!(fixture.client.is_active(&token_id).unwrap());

    // Verify total supply incremented
    let supply = fixture.client.total_supply().unwrap();
    assert_eq!(supply, 1);
}

#[test]
fn test_mint_multiple() {
    let fixture = TestFixture::setup();
    
    for i in 0..5 {
        let commitment_id = if i == 0 {
            String::from_str(&fixture.env, "commitment_0")
        } else if i == 1 {
            String::from_str(&fixture.env, "commitment_1")
        } else if i == 2 {
            String::from_str(&fixture.env, "commitment_2")
        } else if i == 3 {
            String::from_str(&fixture.env, "commitment_3")
        } else {
            String::from_str(&fixture.env, "commitment_4")
        };
        let token_id = fixture.client.mint(
            &fixture.admin,
            &fixture.owner,
            &commitment_id,
            &30,
            &10,
            &String::from_str(&fixture.env, "aggressive"),
            &1000_0000000,
            &Address::generate(&fixture.env),
        ).unwrap();
        assert_eq!(token_id, i + 1);
    }
    
    let supply = fixture.client.total_supply().unwrap();
    assert_eq!(supply, 5);
}

#[test]
fn test_mint_sequential_token_ids() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);

    let token_id_1 = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_001"),
        &30,
        &10,
        &String::from_str(&fixture.env, "safe"),
        &1000,
        &asset,
    ).unwrap();
    let token_id_2 = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_002"),
        &60,
        &20,
        &String::from_str(&fixture.env, "balanced"),
        &2000,
        &asset,
    ).unwrap();

    assert_eq!(token_id_1, 1);
    assert_eq!(token_id_2, 2);
    assert_eq!(fixture.client.total_supply().unwrap(), 2);
}

#[test]
fn test_mint_unauthorized_fails() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);
    let unauthorized = Address::generate(&fixture.env);

    let result = fixture.client.try_mint(
        &unauthorized,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_001"),
        &30,
        &10,
        &String::from_str(&fixture.env, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_authorized_minter() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);
    let minter = Address::generate(&fixture.env);

    fixture.client.add_authorized_minter(&fixture.admin, &minter).unwrap();

    let token_id = fixture.client.mint(
        &minter,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_001"),
        &30,
        &10,
        &String::from_str(&fixture.env, "safe"),
        &1000,
        &asset,
    ).unwrap();

    assert_eq!(token_id, 1);
}

#[test]
fn test_mint_invalid_duration_fails() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);

    let result = fixture.client.try_mint(
        &fixture.admin,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_001"),
        &0, // Invalid: duration must be > 0
        &10,
        &String::from_str(&fixture.env, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_max_loss_fails() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);

    let result = fixture.client.try_mint(
        &fixture.admin,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_001"),
        &30,
        &101, // Invalid: max_loss must be 0-100
        &String::from_str(&fixture.env, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_commitment_type_fails() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);

    let result = fixture.client.try_mint(
        &fixture.admin,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_001"),
        &30,
        &10,
        &String::from_str(&fixture.env, "invalid_type"), // Invalid
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_amount_fails() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);

    let result = fixture.client.try_mint(
        &fixture.admin,
        &fixture.owner,
        &String::from_str(&fixture.env, "commitment_001"),
        &30,
        &10,
        &String::from_str(&fixture.env, "safe"),
        &0, // Invalid: amount must be > 0
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_all_commitment_types() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);

    // Test "safe"
    let t1 = fixture.client.mint(
        &fixture.admin, &fixture.owner, &String::from_str(&fixture.env, "c1"),
        &30, &10, &String::from_str(&fixture.env, "safe"), &1000, &asset,
    ).unwrap();
    assert_eq!(t1, 1);

    // Test "balanced"
    let t2 = fixture.client.mint(
        &fixture.admin, &fixture.owner, &String::from_str(&fixture.env, "c2"),
        &30, &10, &String::from_str(&fixture.env, "balanced"), &1000, &asset,
    ).unwrap();
    assert_eq!(t2, 2);

    // Test "aggressive"
    let t3 = fixture.client.mint(
        &fixture.admin, &fixture.owner, &String::from_str(&fixture.env, "c3"),
        &30, &10, &String::from_str(&fixture.env, "aggressive"), &1000, &asset,
    ).unwrap();
    assert_eq!(t3, 3);
}

#[test]
fn test_get_metadata() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    let metadata = fixture.client.get_metadata(&token_id).unwrap();
    
    assert_eq!(metadata.commitment_id, commitment_id);
    assert!(metadata.expires_at >= metadata.created_at);
}

#[test]
fn test_get_metadata_not_found() {
    let fixture = TestFixture::setup();
    let result = fixture.client.try_get_metadata(&999);
    assert!(result.is_err());
}

#[test]
fn test_owner_of() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    let owner = fixture.client.owner_of(&token_id).unwrap();
    assert_eq!(owner, fixture.owner);
}

#[test]
fn test_owner_of_not_found() {
    let fixture = TestFixture::setup();
    let result = fixture.client.try_owner_of(&999);
    assert!(result.is_err());
}

#[test]
fn test_transfer() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    // Transfer to user1
    fixture.client.transfer(&fixture.owner, &fixture.user1, &token_id).unwrap();

    let new_owner = fixture.client.owner_of(&token_id).unwrap();
    assert_eq!(new_owner, fixture.user1);
}

#[test]
fn test_transfer_by_non_owner() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    // Try to transfer as user1 (not owner)
    let result = fixture.client.try_transfer(&fixture.user1, &fixture.user2, &token_id);
    assert!(result.is_err());
}

#[test]
fn test_is_active() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    assert!(fixture.client.is_active(&token_id).unwrap());
}

#[test]
fn test_is_active_nonexistent_token() {
    let fixture = TestFixture::setup();
    let result = fixture.client.try_is_active(&999);
    assert!(result.is_err());
}

#[test]
fn test_settle() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    // Fast forward time to after expiration
    let metadata = fixture.client.get_metadata(&token_id).unwrap();
    fixture.env.ledger().with_mut(|li| {
        li.timestamp = metadata.expires_at + 1;
    });

    fixture.client.settle(&token_id).unwrap();

    assert!(!fixture.client.is_active(&token_id).unwrap());
}

#[test]
fn test_settle_before_expiration() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    let result = fixture.client.try_settle(&token_id);
    assert!(result.is_err());
}

#[test]
fn test_settle_nonexistent_token() {
    let fixture = TestFixture::setup();
    let result = fixture.client.try_settle(&999);
    assert!(result.is_err());
}

#[test]
fn test_transfer_after_settle() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    // Fast forward time and settle
    let metadata = fixture.client.get_metadata(&token_id).unwrap();
    fixture.env.ledger().with_mut(|li| {
        li.timestamp = metadata.expires_at + 1;
    });

    fixture.client.settle(&token_id).unwrap();

    // Try to transfer after settlement
    let result = fixture.client.try_transfer(&fixture.owner, &fixture.user1, &token_id);
    assert!(result.is_err());
}

// Edge Case Tests

#[test]
fn test_mint_with_max_values() {
    let fixture = TestFixture::setup();
    let asset = Address::generate(&fixture.env);
    
    // Test with max values
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &String::from_str(&fixture.env, "test_commitment"),
        &u32::MAX,
        &100,
        &String::from_str(&fixture.env, "aggressive"),
        &i128::MAX,
        &asset,
    ).unwrap();
    assert_eq!(token_id, 1);
}

// Event Emission Tests

#[test]
fn test_mint_emits_event() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let _token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    // Check events
    let events = fixture.env.events().all();
    assert!(events.len() > 0);
    // The event should contain mint information
}

#[test]
fn test_transfer_emits_event() {
    let fixture = TestFixture::setup();
    let (commitment_id, duration, max_loss, c_type, amount, asset) = fixture.create_test_metadata();
    
    let token_id = fixture.client.mint(
        &fixture.admin,
        &fixture.owner,
        &commitment_id,
        &duration,
        &max_loss,
        &c_type,
        &amount,
        &asset,
    ).unwrap();

    fixture.client.transfer(&fixture.owner, &fixture.user1, &token_id).unwrap();

    // Check events
    let events = fixture.env.events().all();
    assert!(events.len() > 1); // Mint + Transfer events
}
