#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, symbol_short, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String, // "safe", "balanced", "aggressive"
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub commitment_id: u32,  // Changed from String to u32 for uniqueness
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String, // "active", "settled", "violated", "early_exit"
}

#[contract]
pub struct CommitmentCoreContract;

// Storage keys - using Symbol for efficient storage (max 9 chars)
fn commitment_key(_e: &Env) -> Symbol {
    symbol_short!("Commit")
}

// Storage helpers
fn read_commitment(e: &Env, commitment_id: u32) -> Option<Commitment> {
    let key = (commitment_key(e), commitment_id);
    e.storage().persistent().get(&key)
}

fn set_commitment(e: &Env, commitment: &Commitment) {
    let key = (commitment_key(e), commitment.commitment_id);
    e.storage().persistent().set(&key, commitment);
}

#[contractimpl]
impl CommitmentCoreContract {
    /// Initialize the core commitment contract
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) {
        // Store admin and NFT contract address
        e.storage().instance().set(&symbol_short!("ADMIN"), &admin);
        e.storage().instance().set(&symbol_short!("NFT"), &nft_contract);
        // Initialize counter
        e.storage().instance().set(&symbol_short!("CNTR"), &0u32);
    }

    /// Create a new commitment - returns unique commitment_id (u32)
    pub fn create_commitment(
        e: Env,
        owner: Address,
        amount: i128,
        asset_address: Address,
        rules: CommitmentRules,
    ) -> u32 {
        // Get and increment commitment counter
        let counter_key = symbol_short!("CNTR");
        let counter: u32 = e.storage().instance().get(&counter_key).unwrap_or(0);
        let new_counter = counter + 1;
        e.storage().instance().set(&counter_key, &new_counter);
        
        // The commitment_id IS the counter - guaranteed unique
        let commitment_id = new_counter;
        
        // Calculate expiration time
        let timestamp = e.ledger().timestamp();
        let created_at = timestamp;
        let expires_at = created_at + (rules.duration_days as u64 * 86400); // days to seconds
        
        // Create commitment
        let commitment = Commitment {
            commitment_id,
            owner: owner.clone(),
            nft_token_id: 0, // TODO: Mint NFT and get token ID
            rules: rules.clone(),
            amount,
            asset_address: asset_address.clone(),
            created_at,
            expires_at,
            current_value: amount, // Initially same as amount
            status: String::from_str(&e, "active"),
        };
        
        // Store commitment
        set_commitment(&e, &commitment);
        
        // TODO: Transfer assets from owner to contract
        // TODO: Call NFT contract to mint Commitment NFT
        // TODO: Emit creation event
        
        commitment_id
    }

    /// Get commitment details
    pub fn get_commitment(e: Env, commitment_id: u32) -> Commitment {
        read_commitment(&e, commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"))
    }

    /// Update commitment value (called by allocation logic)
    pub fn update_value(e: Env, commitment_id: u32, new_value: i128) {
        let mut commitment = read_commitment(&e, commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"));
        
        // Update current_value
        commitment.current_value = new_value;
        
        // Store updated commitment
        set_commitment(&e, &commitment);
    }

    /// Check if commitment rules are violated
    pub fn check_violations(e: Env, commitment_id: u32) -> bool {
        let commitment = read_commitment(&e, commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"));

        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            return false;
        }

        let current_time = e.ledger().timestamp();
        let loss_amount = commitment.amount - commitment.current_value;
        let loss_percent = if commitment.amount > 0 {
            (loss_amount * 100) / commitment.amount
        } else {
            0
        };

        let max_loss = commitment.rules.max_loss_percent as i128;
        let loss_violated = loss_percent > max_loss;
        let duration_violated = current_time >= commitment.expires_at;

        loss_violated || duration_violated
    }

    /// Get detailed violation information
    pub fn get_violation_details(e: Env, commitment_id: u32) -> (bool, bool, bool, i128, u64) {
        let commitment = read_commitment(&e, commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"));

        let current_time = e.ledger().timestamp();
        let loss_amount = commitment.amount - commitment.current_value;
        let loss_percent = if commitment.amount > 0 {
            (loss_amount * 100) / commitment.amount
        } else {
            0
        };

        let max_loss = commitment.rules.max_loss_percent as i128;
        let loss_violated = loss_percent > max_loss;
        let duration_violated = current_time >= commitment.expires_at;
        let time_remaining = if current_time < commitment.expires_at {
            commitment.expires_at - current_time
        } else {
            0
        };

        let has_violations = loss_violated || duration_violated;
        (has_violations, loss_violated, duration_violated, loss_percent, time_remaining)
    }

    /// Settle commitment at maturity
    pub fn settle(e: Env, commitment_id: u32) {
        let mut commitment = read_commitment(&e, commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"));
        
        let current_time = e.ledger().timestamp();
        if current_time < commitment.expires_at {
            panic!("Commitment not yet expired");
        }
        
        commitment.status = String::from_str(&e, "settled");
        set_commitment(&e, &commitment);
    }

    /// Early exit (with penalty)
    pub fn early_exit(e: Env, commitment_id: u32, caller: Address) {
        caller.require_auth();
        
        let mut commitment = read_commitment(&e, commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"));
        
        // Verify caller is owner
        if commitment.owner != caller {
            panic!("Unauthorized");
        }
        
        commitment.status = String::from_str(&e, "early_exit");
        set_commitment(&e, &commitment);
    }

    /// Allocate liquidity (called by allocation strategy)
    pub fn allocate(_e: Env, _commitment_id: u32, _target_pool: Address, _amount: i128) {
        // TODO: Implement allocation logic
    }
}

#[cfg(test)]
mod tests;
