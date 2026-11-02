use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
    IntoVal, TryFromVal,
};

fn setup(supply: i128) -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|info| info.sequence_number = 100);
    let id = env.register_contract(None, TokenContract);
    let admin = Address::generate(&env);
    TokenContractClient::new(&env, &id).initialize(
        &admin,
        &String::from_str(&env, "Lens Token"),
        &String::from_str(&env, "LENS"),
        &7,
        &supply,
    );
    (env, id, admin)
}

#[test]
fn initialization_sets_metadata_and_supply_once() {
    let (env, id, admin) = setup(1000);
    let client = TokenContractClient::new(&env, &id);
    assert_eq!(client.name(), String::from_str(&env, "Lens Token"));
    assert_eq!(client.symbol(), String::from_str(&env, "LENS"));
    assert_eq!(client.decimals(), 7);
    assert_eq!(client.total_supply(), 1000);
    assert_eq!(client.balance(&admin), 1000);
    let attacker = Address::generate(&env);
    assert_eq!(
        client.try_initialize(
            &attacker,
            &String::from_str(&env, "Other"),
            &String::from_str(&env, "BAD"),
            &0,
            &500
        ),
        Err(Ok(TokenError::AlreadyInitialized))
    );
    assert_eq!(client.balance(&admin), 1000);
    assert_eq!(client.balance(&attacker), 0);
    assert_eq!(client.total_supply(), 1000);
}

#[test]
fn invalid_initialization_does_not_write_state() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, TokenContract);
    let client = TokenContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    let name = String::from_str(&env, "Lens");
    assert_eq!(
        client.try_initialize(&admin, &name, &name, &7, &-1),
        Err(Ok(TokenError::NegativeAmount))
    );
    assert_eq!(
        client.try_initialize(&admin, &name, &name, &19, &1),
        Err(Ok(TokenError::InvalidDecimals))
    );
    assert_eq!(client.total_supply(), 0);
    assert_eq!(client.try_name(), Err(Ok(TokenError::NotInitialized)));
    client.initialize(&admin, &name, &name, &18, &0);
    assert_eq!(client.decimals(), 18);
    assert_eq!(client.total_supply(), 0);
}

#[test]
fn transfers_preserve_supply_and_handle_self_and_zero_transfers() {
    let (env, id, admin) = setup(i128::MAX);
    let client = TokenContractClient::new(&env, &id);
    let recipient = Address::generate(&env);
    client.transfer(&admin, &admin, &i128::MAX);
    assert_eq!(client.balance(&admin), i128::MAX);
    client.transfer(&admin, &recipient, &10);
    client.transfer(&recipient, &admin, &0);
    assert_eq!(client.balance(&admin), i128::MAX - 10);
    assert_eq!(client.balance(&recipient), 10);
    assert_eq!(client.total_supply(), i128::MAX);
    assert_eq!(
        client.try_transfer(&recipient, &admin, &11),
        Err(Ok(TokenError::InsufficientBalance))
    );
    assert_eq!(
        client.try_transfer(&recipient, &recipient, &11),
        Err(Ok(TokenError::InsufficientBalance))
    );
    assert_eq!(
        client.try_transfer(&admin, &recipient, &-1),
        Err(Ok(TokenError::NegativeAmount))
    );
    assert_eq!(client.balance(&admin), i128::MAX - 10);
    assert_eq!(client.balance(&recipient), 10);
}

#[test]
fn allowance_replaces_and_expires_after_the_boundary_ledger() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    client.approve(&admin, &spender, &50, &110);
    client.approve(&admin, &spender, &20, &110);
    assert_eq!(client.allowance(&admin, &spender), 20);
    env.ledger().with_mut(|info| info.sequence_number = 110);
    assert_eq!(client.allowance(&admin, &spender), 20);
    env.ledger().with_mut(|info| info.sequence_number = 111);
    assert_eq!(client.allowance(&admin, &spender), 0);
    assert_eq!(
        client.try_transfer_from(&spender, &admin, &spender, &1),
        Err(Ok(TokenError::InsufficientAllowance))
    );
}

#[test]
fn approval_rejects_bad_amounts_and_expiration_but_allows_revocation() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    assert_eq!(
        client.try_approve(&admin, &spender, &-1, &110),
        Err(Ok(TokenError::NegativeAmount))
    );
    assert_eq!(
        client.try_approve(&admin, &spender, &1, &99),
        Err(Ok(TokenError::InvalidExpiration))
    );
    assert_eq!(
        client.try_approve(&admin, &spender, &1, &u32::MAX),
        Err(Ok(TokenError::InvalidExpiration))
    );
    client.approve(&admin, &spender, &20, &110);
    client.approve(&admin, &spender, &0, &0);
    assert_eq!(client.allowance(&admin, &spender), 0);
    client.approve(&admin, &spender, &1, &100);
    assert_eq!(client.allowance(&admin, &spender), 1);
}

#[test]
fn delegated_transfer_consumes_allowance_and_preserves_supply() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.approve(&admin, &spender, &50, &110);
    client.transfer_from(&spender, &admin, &recipient, &30);
    assert_eq!(client.allowance(&admin, &spender), 20);
    assert_eq!(client.balance(&admin), 70);
    assert_eq!(client.balance(&recipient), 30);
    assert_eq!(client.total_supply(), 100);
    assert_eq!(
        client.try_transfer_from(&spender, &admin, &recipient, &21),
        Err(Ok(TokenError::InsufficientAllowance))
    );
    assert_eq!(
        client.try_transfer_from(&spender, &admin, &recipient, &-1),
        Err(Ok(TokenError::NegativeAmount))
    );
    assert_eq!(client.allowance(&admin, &spender), 20);
}

#[test]
fn failed_delegated_spend_rolls_back_allowance_and_balances() {
    let (env, id, admin) = setup(10);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    client.approve(&admin, &spender, &50, &110);
    assert_eq!(
        client.try_transfer_from(&spender, &admin, &spender, &11),
        Err(Ok(TokenError::InsufficientBalance))
    );
    assert_eq!(client.allowance(&admin, &spender), 50);
    assert_eq!(
        client.try_burn_from(&spender, &admin, &11),
        Err(Ok(TokenError::InsufficientBalance))
    );
    assert_eq!(client.allowance(&admin, &spender), 50);
    assert_eq!(client.balance(&admin), 10);
    assert_eq!(client.balance(&spender), 0);
    assert_eq!(client.total_supply(), 10);
}

#[test]
fn burn_and_delegated_burn_reduce_supply_and_balance_together() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    client.burn(&admin, &10);
    client.approve(&admin, &spender, &40, &110);
    client.burn_from(&spender, &admin, &30);
    assert_eq!(client.balance(&admin), 60);
    assert_eq!(client.total_supply(), 60);
    assert_eq!(client.allowance(&admin, &spender), 10);
    assert_eq!(
        client.try_burn(&admin, &-1),
        Err(Ok(TokenError::NegativeAmount))
    );
    assert_eq!(
        client.try_burn(&admin, &61),
        Err(Ok(TokenError::InsufficientBalance))
    );
    assert_eq!(client.total_supply(), 60);
    client.burn(&admin, &60);
    assert_eq!(client.balance(&admin), 0);
    assert_eq!(client.total_supply(), 0);
}

#[test]
fn mutations_require_initialization() {
    let env = Env::default();
    let id = env.register_contract(None, TokenContract);
    let client = TokenContractClient::new(&env, &id);
    let address = Address::generate(&env);
    assert_eq!(
        client.try_transfer(&address, &address, &0),
        Err(Ok(TokenError::NotInitialized))
    );
    assert_eq!(
        client.try_approve(&address, &address, &0, &0),
        Err(Ok(TokenError::NotInitialized))
    );
    assert_eq!(
        client.try_transfer_from(&address, &address, &address, &0),
        Err(Ok(TokenError::NotInitialized))
    );
    assert_eq!(
        client.try_burn(&address, &0),
        Err(Ok(TokenError::NotInitialized))
    );
    assert_eq!(
        client.try_burn_from(&address, &address, &0),
        Err(Ok(TokenError::NotInitialized))
    );
}

#[test]
fn unauthenticated_mutations_are_rejected() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    client.approve(&admin, &spender, &40, &110);
    env.mock_auths(&[]);
    assert!(client.try_transfer(&admin, &spender, &1).is_err());
    assert!(client.try_approve(&admin, &spender, &10, &110).is_err());
    assert!(client.try_burn(&admin, &1).is_err());
    assert!(client
        .try_transfer_from(&spender, &admin, &spender, &1)
        .is_err());
    assert!(client.try_burn_from(&spender, &admin, &1).is_err());
    assert_eq!(client.balance(&admin), 100);
    assert_eq!(client.total_supply(), 100);
    assert_eq!(client.allowance(&admin, &spender), 40);
}

#[test]
fn initialization_requires_the_initial_holders_authorization() {
    let env = Env::default();
    let id = env.register_contract(None, TokenContract);
    let client = TokenContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    let name = String::from_str(&env, "Lens");
    assert!(client
        .try_initialize(&admin, &name, &name, &7, &100)
        .is_err());
    assert_eq!(client.total_supply(), 0);
    env.mock_all_auths();
    client.initialize(&admin, &name, &name, &7, &100);
    assert_eq!(client.balance(&admin), 100);
}

#[test]
fn token_events_have_standard_topics_and_values() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let recipient = Address::generate(&env);
    client.transfer(&admin, &recipient, &10);
    let event = env.events().all().last().unwrap();
    assert_eq!(event.0, id);
    assert_eq!(
        event.1,
        (symbol_short!("transfer"), admin.clone(), recipient.clone()).into_val(&env)
    );
    assert_eq!(i128::try_from_val(&env, &event.2).unwrap(), 10);
    client.approve(&admin, &recipient, &20, &110);
    let event = env.events().all().last().unwrap();
    assert_eq!(
        event.1,
        (symbol_short!("approve"), admin.clone(), recipient).into_val(&env)
    );
    assert_eq!(
        <(i128, u32)>::try_from_val(&env, &event.2).unwrap(),
        (20, 110)
    );
    client.burn(&admin, &5);
    let event = env.events().all().last().unwrap();
    assert_eq!(event.1, (symbol_short!("burn"), admin).into_val(&env));
    assert_eq!(i128::try_from_val(&env, &event.2).unwrap(), 5);
}

#[test]
fn approval_accepts_the_maximum_storage_lifetime_and_rejects_the_next_ledger() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    let max_ttl = env.as_contract(&id, || env.storage().max_ttl());
    let last = env.ledger().sequence() + max_ttl;
    client.approve(&admin, &spender, &10, &last);
    assert_eq!(client.allowance(&admin, &spender), 10);
    assert_eq!(
        client.try_approve(&admin, &spender, &20, &(last + 1)),
        Err(Ok(TokenError::InvalidExpiration))
    );
    assert_eq!(client.allowance(&admin, &spender), 10);
}

#[test]
fn delegated_transfer_requires_the_spender_rather_than_the_holder() {
    let (env, id, admin) = setup(100);
    let client = TokenContractClient::new(&env, &id);
    let spender = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.approve(&admin, &spender, &40, &110);
    let args: soroban_sdk::Vec<soroban_sdk::Val> =
        (&spender, &admin, &recipient, 10_i128).into_val(&env);
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &id,
            fn_name: "transfer_from",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    assert!(client
        .try_transfer_from(&spender, &admin, &recipient, &10)
        .is_err());
    assert_eq!(client.allowance(&admin, &spender), 40);
    env.mock_auths(&[MockAuth {
        address: &spender,
        invoke: &MockAuthInvoke {
            contract: &id,
            fn_name: "transfer_from",
            args,
            sub_invokes: &[],
        },
    }]);
    client.transfer_from(&spender, &admin, &recipient, &10);
    assert_eq!(client.allowance(&admin, &spender), 30);
    assert_eq!(client.balance(&recipient), 10);
}
