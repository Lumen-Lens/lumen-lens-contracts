//! Fixed-supply Soroban token fixture for SDK generation and integration tests.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Balance(Address),
    Allowance(Address, Address),
    Admin,
    Name,
    Symbol,
    Decimals,
    TotalSupply,
}

/// Stable error codes included in the generated contract specification.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TokenError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NegativeAmount = 3,
    InsufficientBalance = 4,
    Overflow = 5,
    InvalidDecimals = 6,
    InsufficientAllowance = 7,
    InvalidExpiration = 8,
}

#[contracttype]
#[derive(Clone)]
pub struct AllowanceData {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contract]
pub struct TokenContract;

fn initialized(env: &Env) -> Result<(), TokenError> {
    if !env.storage().instance().has(&DataKey::Admin) {
        return Err(TokenError::NotInitialized);
    }
    Ok(())
}

fn nonnegative(amount: i128) -> Result<(), TokenError> {
    if amount < 0 {
        return Err(TokenError::NegativeAmount);
    }
    Ok(())
}

fn move_balance(env: &Env, from: &Address, to: &Address, amount: i128) -> Result<(), TokenError> {
    nonnegative(amount)?;
    let balance = TokenContract::balance(env.clone(), from.clone());
    if balance < amount {
        return Err(TokenError::InsufficientBalance);
    }
    // A self-transfer still checks funds and authorization, but changes no balances.
    if from != to {
        let recipient = TokenContract::balance(env.clone(), to.clone())
            .checked_add(amount)
            .ok_or(TokenError::Overflow)?;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(balance - amount));
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &recipient);
    }
    env.events().publish(
        (symbol_short!("transfer"), from.clone(), to.clone()),
        amount,
    );
    Ok(())
}

fn burn_balance(env: &Env, from: &Address, amount: i128) -> Result<(), TokenError> {
    nonnegative(amount)?;
    let balance = TokenContract::balance(env.clone(), from.clone());
    if balance < amount {
        return Err(TokenError::InsufficientBalance);
    }
    let supply = TokenContract::total_supply(env.clone())
        .checked_sub(amount)
        .ok_or(TokenError::Overflow)?;
    env.storage()
        .persistent()
        .set(&DataKey::Balance(from.clone()), &(balance - amount));
    env.storage().instance().set(&DataKey::TotalSupply, &supply);
    env.events()
        .publish((symbol_short!("burn"), from.clone()), amount);
    Ok(())
}

fn spend_allowance(
    env: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
) -> Result<(), TokenError> {
    nonnegative(amount)?;
    let available = TokenContract::allowance(env.clone(), from.clone(), spender.clone());
    if available < amount {
        return Err(TokenError::InsufficientAllowance);
    }
    if amount > 0 {
        let key = DataKey::Allowance(from.clone(), spender.clone());
        let mut allowance: AllowanceData = env
            .storage()
            .temporary()
            .get(&key)
            .ok_or(TokenError::InsufficientAllowance)?;
        allowance.amount -= amount;
        env.storage().temporary().set(&key, &allowance);
    }
    Ok(())
}

#[contractimpl]
impl TokenContract {
    /// Initialize once, authorizing the initial holder and validating the supply.
    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        initial_supply: i128,
    ) -> Result<(), TokenError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(TokenError::AlreadyInitialized);
        }
        admin.require_auth();
        nonnegative(initial_supply)?;
        if decimals > 18 {
            return Err(TokenError::InvalidDecimals);
        }
        let store = env.storage().instance();
        store.set(&DataKey::Admin, &admin);
        store.set(&DataKey::Name, &name);
        store.set(&DataKey::Symbol, &symbol);
        store.set(&DataKey::Decimals, &decimals);
        store.set(&DataKey::TotalSupply, &initial_supply);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(admin.clone()), &initial_supply);
        env.events().publish(
            (symbol_short!("mint"), admin.clone(), admin),
            initial_supply,
        );
        Ok(())
    }

    /// Return the account balance, or zero for an account without a balance.
    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(account))
            .unwrap_or(0)
    }

    /// Transfer a nonnegative amount, authorized by the sender.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), TokenError> {
        initialized(&env)?;
        from.require_auth();
        move_balance(&env, &from, &to, amount)
    }

    /// Replace a spender's allowance. Expiration is inclusive of the given ledger.
    pub fn approve(
        env: Env,
        from: Address,
        spender: Address,
        amount: i128,
        expiration_ledger: u32,
    ) -> Result<(), TokenError> {
        initialized(&env)?;
        from.require_auth();
        nonnegative(amount)?;
        let ledger = env.ledger().sequence();
        if amount > 0
            && (expiration_ledger < ledger || expiration_ledger - ledger > env.storage().max_ttl())
        {
            return Err(TokenError::InvalidExpiration);
        }
        let key = DataKey::Allowance(from.clone(), spender.clone());
        if amount == 0 {
            env.storage().temporary().remove(&key);
        } else {
            env.storage().temporary().set(
                &key,
                &AllowanceData {
                    amount,
                    expiration_ledger,
                },
            );
            let ttl = expiration_ledger - ledger;
            env.storage().temporary().extend_ttl(&key, ttl, ttl);
        }
        env.events().publish(
            (symbol_short!("approve"), from, spender),
            (amount, expiration_ledger),
        );
        Ok(())
    }

    /// Return the unexpired allowance, or zero if absent or expired.
    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        let allowance: Option<AllowanceData> = env
            .storage()
            .temporary()
            .get(&DataKey::Allowance(from, spender));
        allowance
            .filter(|value| value.expiration_ledger >= env.ledger().sequence())
            .map(|value| value.amount)
            .unwrap_or(0)
    }

    /// Transfer using the spender's authorization and consume their allowance.
    pub fn transfer_from(
        env: Env,
        spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), TokenError> {
        initialized(&env)?;
        spender.require_auth();
        spend_allowance(&env, &from, &spender, amount)?;
        move_balance(&env, &from, &to, amount)
    }

    /// Burn tokens held by the authorized account, reducing total supply.
    pub fn burn(env: Env, from: Address, amount: i128) -> Result<(), TokenError> {
        initialized(&env)?;
        from.require_auth();
        burn_balance(&env, &from, amount)
    }

    /// Burn using an authorized spender's allowance.
    pub fn burn_from(
        env: Env,
        spender: Address,
        from: Address,
        amount: i128,
    ) -> Result<(), TokenError> {
        initialized(&env)?;
        spender.require_auth();
        spend_allowance(&env, &from, &spender, amount)?;
        burn_balance(&env, &from, amount)
    }

    pub fn name(env: Env) -> Result<String, TokenError> {
        initialized(&env)?;
        Ok(env.storage().instance().get(&DataKey::Name).unwrap())
    }

    pub fn symbol(env: Env) -> Result<String, TokenError> {
        initialized(&env)?;
        Ok(env.storage().instance().get(&DataKey::Symbol).unwrap())
    }

    pub fn decimals(env: Env) -> Result<u32, TokenError> {
        initialized(&env)?;
        Ok(env.storage().instance().get(&DataKey::Decimals).unwrap())
    }

    pub fn total_supply(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test;
