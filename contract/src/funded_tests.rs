//! Funded mode: custodial balances backed one-for-one by real tokens.
//! Invariant checked throughout: supply == tokens the contract must hold.
use crate::engine::Engine;
use crate::Contract;
use near_sdk::json_types::U128;
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{testing_env, AccountId};
use std::collections::BTreeMap;
const SELF: &str = "contract.test";

fn empty() -> Engine {
    Engine::new(SELF.into(), BTreeMap::new(), 0).unwrap()
}
fn funded_grant() -> Engine {
    let s = empty().deposit("dao.treasury", 365_000).unwrap();
    let s = s.ensure_wallet("alice").unwrap();
    s.create_grant("dao.treasury", "g1", "Core".into(), "alice", 365_000, 0, 90, 365, false)
        .unwrap()
}
fn at(mut s: Engine, day: u64) -> Engine {
    s.advance_to(day).unwrap();
    s
}

#[test]
fn deposit_grows_supply_by_exactly_the_tokens_received() {
    let s = funded_grant();
    assert_eq!(s.supply, 365_000);
    assert_eq!(s.balance("dao.treasury").unwrap(), 0);
    assert_eq!(s.grant("g1").unwrap().escrow, 365_000);
}
#[test]
fn unlock_pays_the_lender_first_then_the_holder_inside_the_contract() {
    let s = funded_grant().deposit("lena.lender", 100_000).unwrap();
    let s = s.borrow("alice", "g1", 50_000, "lena.lender").unwrap();
    let debt = s.grant("g1").unwrap().lien.as_ref().unwrap().debt;
    let s = at(s, 200).unlock_internal("alice", "g1").unwrap();
    let vested = 365_000 * 200 / 365;
    assert_eq!(s.balance("lena.lender").unwrap(), 100_000 - 50_000 + debt);
    assert_eq!(s.balance("alice").unwrap(), 50_000 + vested - debt);
    assert_eq!(s.supply, 365_000 + 100_000);
}
#[test]
fn withdraw_shrinks_supply_and_a_failed_transfer_restores_it() {
    let s = at(funded_grant(), 200).unlock_internal("alice", "g1").unwrap();
    let bal = s.balance("alice").unwrap();
    let w = s.withdraw_reserve("alice", bal).unwrap();
    assert_eq!(w.balance("alice").unwrap(), 0);
    assert_eq!(w.supply, s.supply - bal);
    assert!(w.withdraw_reserve("alice", 1).is_err());
    assert!(w.withdraw_restore("alice", "alice", bal).is_err());
    let back = w.withdraw_restore(SELF, "alice", bal).unwrap();
    assert_eq!(back.balance("alice").unwrap(), bal);
    assert_eq!(back.supply, s.supply);
}
#[test]
fn nothing_unlocks_before_the_cliff_and_no_one_else_can_unlock() {
    let s = at(funded_grant(), 30);
    assert!(s.unlock_internal("alice", "g1").is_err());
    let s = at(s, 200);
    assert!(s.unlock_internal("mallory", "g1").is_err());
}

fn ctx(predecessor: &str) {
    let mut b = VMContextBuilder::new();
    b.current_account_id("contract.test".parse().unwrap())
        .predecessor_account_id(predecessor.parse().unwrap())
        .block_timestamp(0);
    testing_env!(b.build());
}
fn funded_contract() -> Contract {
    ctx("deployer.test");
    Contract::new_funded("token.test".parse().unwrap())
}
fn grant_msg() -> String {
    r#"{"action":"create_grant","grant_id":"g1","title":"Core","holder":"alice.test","start":0,"cliff":90,"duration":365,"cancellable":false}"#.into()
}

#[test]
fn the_configured_token_creates_a_grant_backed_by_the_tokens_sent() {
    let mut c = funded_contract();
    ctx("token.test");
    let refund = c.ft_on_transfer("dao.test".parse().unwrap(), U128(365_000), grant_msg());
    assert_eq!(refund.0, 0);
    assert_eq!(c.backing().0, 365_000);
    let alice: AccountId = "alice.test".parse().unwrap();
    assert_eq!(c.get_grants(alice).len(), 1);
}
#[test]
#[should_panic(expected = "only the configured token can deposit")]
fn another_token_cannot_deposit() {
    let mut c = funded_contract();
    ctx("fake-token.test");
    c.ft_on_transfer("dao.test".parse().unwrap(), U128(365_000), grant_msg());
}
#[test]
#[should_panic(expected = "the funded contract uses unlock, then withdraw")]
fn funded_contract_refuses_the_demo_claim() {
    let mut c = funded_contract();
    let _ = c.claim("g1".into());
}
#[test]
#[should_panic(expected = "staking pool not wired")]
fn funded_contract_refuses_unbacked_staking() {
    let mut c = funded_contract();
    c.stake("g1".into());
}
