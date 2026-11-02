//! Sample Counter Contract — used as a fixture for Lumen Lens SDK generation tests.
#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Env};

/// Storage key for the counter value.
#[contracttype]
pub enum DataKey {
    Counter,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CounterError {
    Overflow = 1,
}

#[contract]
pub struct CounterContract;

#[contractimpl]
impl CounterContract {
    /// Increment the counter by `amount` and return the new value.
    pub fn increment(env: Env, amount: u32) -> Result<u32, CounterError> {
        let current: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0);
        let next = current.checked_add(amount).ok_or(CounterError::Overflow)?;
        env.storage().persistent().set(&DataKey::Counter, &next);
        Ok(next)
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
    fn overflow_preserves_the_previous_value() {
        let env = Env::default();
        let id = env.register_contract(None, CounterContract);
        let client = CounterContractClient::new(&env, &id);
        assert_eq!(client.get(), 0);
        assert_eq!(client.increment(&0), 0);
        assert_eq!(client.increment(&u32::MAX), u32::MAX);
        assert_eq!(client.try_increment(&1), Err(Ok(CounterError::Overflow)));
        assert_eq!(client.get(), u32::MAX);
        client.reset();
        assert_eq!(client.increment(&1), 1);
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
