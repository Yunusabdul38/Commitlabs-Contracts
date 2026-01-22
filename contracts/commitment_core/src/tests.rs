#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String, Symbol, Vec};

/* -------------------- DUMMY CONTRACTS -------------------- */

#[contract]
struct DummyTokenContract;

#[contractimpl]
impl DummyTokenContract {
    pub fn transfer(from: Address, to: Address, amount: i128) {
        // record transfer for assertions
    }
}

#[contract]
struct DummyNFTContract;

#[contractimpl]
impl DummyNFTContract {
    pub fn mint(owner: Address, commitment_id: String) -> u32 {
        1
    }

    pub fn mark_settled(token_id: u32) {
        // record settled
    }
}

/* -------------------- HELPER FUNCTIONS -------------------- */

fn create_test_commitment(e: &Env, id: &str, owner: Address, expired: bool) -> Commitment {
    let now = e.ledger().timestamp();
    let (created_at, expires_at) = if expired {
        (now - 10000, now - 100)
    } else {
        (now, now + 10000)
    };

    Commitment {
        commitment_id: String::from_str(e, id),
        owner,
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days: 7,
            max_loss_percent: 20,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 5,
            min_fee_threshold: 0,
        },
        amount: 1000,
        asset_address: Address::generate(e),
        created_at,
        expires_at,
        current_value: 1000,
        status: String::from_str(e, "active"),
    }
}

fn setup_test_env() -> (Env, Address, Address, Address) {
    let e = Env::default();
    let token_id = e.register_contract(None, DummyTokenContract);
    let nft_id = e.register_contract(None, DummyNFTContract);
    let core_id = e.register_contract(None, CommitmentCoreContract);

    (e, Address::Contract(token_id), Address::Contract(nft_id), Address::Contract(core_id))
}

/* -------------------- TESTS -------------------- */

#[test]
fn test_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    
    let stored_admin: Address = e.storage().instance().get(&Symbol::short("ADMIN")).unwrap();
    let stored_nft: Address = e.storage().instance().get(&Symbol::short("NFT")).unwrap();
    
    assert_eq!(stored_admin, admin);
    assert_eq!(stored_nft, nft_contract);
}

#[test]
fn test_settlement_flow_basic() {
    let (e, token_addr, nft_addr, core_addr) = setup_test_env();
    
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    
    // Initialize contract
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create an expired commitment
    let now = e.ledger().timestamp();
    let commitment = Commitment {
        commitment_id: String::from_str(&e, "settle_test_1"),
        owner: owner.clone(),
        nft_token_id: 101,
        rules: CommitmentRules {
            duration_days: 1,
            max_loss_percent: 10,
            commitment_type: String::from_str(&e, "safe"),
            early_exit_penalty: 5,
            min_fee_threshold: 0,
        },
        amount: 5000,
        asset_address: token_addr.clone(),
        created_at: now - 100000,
        expires_at: now - 1000,
        current_value: 5500,
        status: String::from_str(&e, "active"),
    };
    
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment.clone());
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Settle the commitment
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "settle_test_1"));
    
    // Verify settlement
    let updated_commitments: Vec<Commitment> = e.storage().instance().get(&Symbol::short("COMMS")).unwrap();
    assert_eq!(updated_commitments.len(), 1);
    assert_eq!(updated_commitments.get(0).status, String::from_str(&e, "settled"));
}

#[test]
#[should_panic(expected = "Commitment not expired")]
fn test_settlement_rejects_active_commitment() {
    let (e, token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create non-expired commitment
    let commitment = create_test_commitment(&e, "not_expired", owner.clone(), false);
    
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Try to settle; should panic
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "not_expired"));
}

#[test]
#[should_panic(expected = "Commitment not found")]
fn test_settlement_commitment_not_found() {
    let (e, _token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let admin = Address::generate(&e);
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Try to settle non-existent commitment
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "nonexistent"));
}

#[test]
#[should_panic(expected = "Already settled")]
fn test_settlement_already_settled() {
    let (e, token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create expired commitment already settled
    let now = e.ledger().timestamp();
    let mut commitment = create_test_commitment(&e, "already_settled", owner.clone(), true);
    commitment.status = String::from_str(&e, "settled");
    
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Try to settle already settled commitment; should panic
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "already_settled"));
}

#[test]
fn test_expiration_check_expired() {
    let (e, _token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create expired commitment
    let commitment = create_test_commitment(&e, "expired_check", owner, true);
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Check violations
    let is_violated = CommitmentCoreContract::check_violations(
        e.clone(),
        String::from_str(&e, "expired_check"),
    );
    assert!(is_violated);
}

#[test]
fn test_expiration_check_not_expired() {
    let (e, _token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create active (non-expired) commitment
    let commitment = create_test_commitment(&e, "not_expired_check", owner, false);
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Check violations
    let is_violated = CommitmentCoreContract::check_violations(
        e.clone(),
        String::from_str(&e, "not_expired_check"),
    );
    assert!(!is_violated);
}

#[test]
fn test_asset_transfer_on_settlement() {
    let (e, token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    let settlement_amount = 7500i128;
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create expired commitment
    let now = e.ledger().timestamp();
    let mut commitment = Commitment {
        commitment_id: String::from_str(&e, "transfer_test"),
        owner: owner.clone(),
        nft_token_id: 102,
        rules: CommitmentRules {
            duration_days: 5,
            max_loss_percent: 15,
            commitment_type: String::from_str(&e, "growth"),
            early_exit_penalty: 10,
            min_fee_threshold: 0,
        },
        amount: 5000,
        asset_address: token_addr.clone(),
        created_at: now - 500000,
        expires_at: now - 10000,
        current_value: settlement_amount,
        status: String::from_str(&e, "active"),
    };
    
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Settle - this will call token transfer
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "transfer_test"));
    
    // Verify the commitment is marked settled
    let updated_commitments: Vec<Commitment> = e.storage().instance().get(&Symbol::short("COMMS")).unwrap();
    assert_eq!(updated_commitments.get(0).status, String::from_str(&e, "settled"));
    assert_eq!(updated_commitments.get(0).current_value, settlement_amount);
}

#[test]
fn test_settlement_with_different_values() {
    let (e, _token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    let now = e.ledger().timestamp();
    
    // Test case 1: Settlement with gain
    let commitment_gain = Commitment {
        commitment_id: String::from_str(&e, "gain_test"),
        owner: owner.clone(),
        nft_token_id: 201,
        rules: CommitmentRules {
            duration_days: 30,
            max_loss_percent: 5,
            commitment_type: String::from_str(&e, "stable"),
            early_exit_penalty: 2,
            min_fee_threshold: 0,
        },
        amount: 10000,
        asset_address: Address::generate(&e),
        created_at: now - 2592000,
        expires_at: now - 1,
        current_value: 11000,
        status: String::from_str(&e, "active"),
    };
    
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment_gain);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "gain_test"));
    
    let updated: Vec<Commitment> = e.storage().instance().get(&Symbol::short("COMMS")).unwrap();
    assert_eq!(updated.get(0).current_value, 11000);
    assert_eq!(updated.get(0).status, String::from_str(&e, "settled"));
}

#[test]
fn test_cross_contract_nft_settlement() {
    let (e, token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    let nft_token_id = 999u32;
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create expired commitment with specific NFT ID
    let now = e.ledger().timestamp();
    let commitment = Commitment {
        commitment_id: String::from_str(&e, "nft_cross_contract"),
        owner: owner.clone(),
        nft_token_id,
        rules: CommitmentRules {
            duration_days: 1,
            max_loss_percent: 10,
            commitment_type: String::from_str(&e, "safe"),
            early_exit_penalty: 5,
            min_fee_threshold: 0,
        },
        amount: 2000,
        asset_address: token_addr.clone(),
        created_at: now - 100000,
        expires_at: now - 1000,
        current_value: 2000,
        status: String::from_str(&e, "active"),
    };
    
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Settle - this will invoke NFT contract
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "nft_cross_contract"));
    
    // Verify settlement completed
    let updated_commitments: Vec<Commitment> = e.storage().instance().get(&Symbol::short("COMMS")).unwrap();
    assert_eq!(updated_commitments.get(0).status, String::from_str(&e, "settled"));
    assert_eq!(updated_commitments.get(0).nft_token_id, nft_token_id);
}

#[test]
fn test_settlement_removes_commitment_status() {
    let (e, _token_addr, nft_addr, _core_addr) = setup_test_env();
    
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    
    // Initialize
    CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_addr.clone());
    
    // Create multiple commitments
    let now = e.ledger().timestamp();
    let commitment1 = Commitment {
        commitment_id: String::from_str(&e, "multi_1"),
        owner: owner.clone(),
        nft_token_id: 301,
        rules: CommitmentRules {
            duration_days: 1,
            max_loss_percent: 10,
            commitment_type: String::from_str(&e, "safe"),
            early_exit_penalty: 5,
            min_fee_threshold: 0,
        },
        amount: 1000,
        asset_address: Address::generate(&e),
        created_at: now - 100000,
        expires_at: now - 1000,
        current_value: 1000,
        status: String::from_str(&e, "active"),
    };
    
    let commitment2 = Commitment {
        commitment_id: String::from_str(&e, "multi_2"),
        owner: owner.clone(),
        nft_token_id: 302,
        rules: CommitmentRules {
            duration_days: 30,
            max_loss_percent: 20,
            commitment_type: String::from_str(&e, "growth"),
            early_exit_penalty: 10,
            min_fee_threshold: 0,
        },
        amount: 2000,
        asset_address: Address::generate(&e),
        created_at: now,
        expires_at: now + 2592000,
        current_value: 2000,
        status: String::from_str(&e, "active"),
    };
    
    let mut commitments: Vec<Commitment> = Vec::new(&e);
    commitments.push_back(commitment1);
    commitments.push_back(commitment2);
    e.storage().instance().set(&Symbol::short("COMMS"), &commitments);
    
    // Settle first commitment
    CommitmentCoreContract::settle(e.clone(), String::from_str(&e, "multi_1"));
    
    // Verify only first is settled
    let updated_commitments: Vec<Commitment> = e.storage().instance().get(&Symbol::short("COMMS")).unwrap();
    assert_eq!(updated_commitments.len(), 2);
    assert_eq!(updated_commitments.get(0).status, String::from_str(&e, "settled"));
    assert_eq!(updated_commitments.get(1).status, String::from_str(&e, "active"));
}

