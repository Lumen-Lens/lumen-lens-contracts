//! Sample Counter Contract — used as a fixture for Lumen Lens SDK generation tests.
#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Env};

/// Storage key for the counter value.
#[contracttype]
pub enum DataKey {
    Counter,
}

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    /// Increment the counter by `amount` and return the new value.
    pub fn increment(env: Env, amount: u32) -> u32 {
        let current: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0);
        let next = current + amount;
        env.storage().persistent().set(&DataKey::Counter, &next);
        next
    }

    /// Return the current counter value.
    pub fn get(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0)
    }

    /// Reset the counter to zero.
    pub fn reset(env: Env) {
        env.storage().persistent().remove(&DataKey::Counter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_increment_and_get() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CounterContract);
        let client = CounterContractClient::new(&env, &contract_id);

        assert_eq!(client.increment(&5), 5);
        assert_eq!(client.increment(&3), 8);
        assert_eq!(client.get(), 8);
    }

    #[test]
    fn test_reset() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CounterContract);
        let client = CounterContractClient::new(&env, &contract_id);

        client.increment(&10);
        client.reset();
        assert_eq!(client.get(), 0);
    }
}
