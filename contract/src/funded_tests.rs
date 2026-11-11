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
    let s = s.set_lendable("lena.lender", 50_000).unwrap();
    let s = s.borrow("alice", "g1", 50_000, "lena.lender").unwrap();
    assert_eq!(s.lendable_of("lena.lender"), 0);
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
    ctx("contract.test");
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

#[test]
fn a_holder_cannot_borrow_from_someone_who_never_offered() {
    let s = funded_grant().deposit("victim.near", 100_000).unwrap();
    let before = s.clone();
    assert!(s.borrow("alice", "g1", 10_000, "victim.near").is_err());
    let s2 = s.ensure_wallet("bob").unwrap();
    assert!(s2.sell_slice("alice", "g1", 10_000, 10_000, "victim.near").is_err());
    assert_eq!(s, before);
}
#[test]
fn a_buyer_pays_only_up_to_the_offer() {
    let s = funded_grant().deposit("bea.buyer", 100_000).unwrap();
    let s = s.set_lendable("bea.buyer", 6_000).unwrap();
    assert!(s.sell_slice("alice", "g1", 10_000, 6_001, "bea.buyer").is_err());
    let s = s.sell_slice("alice", "g1", 10_000, 6_000, "bea.buyer").unwrap();
    assert_eq!(s.balance("bea.buyer").unwrap(), 94_000);
    assert_eq!(s.lendable_of("bea.buyer"), 0);
}
#[test]
fn an_empty_unreferenced_wallet_makes_room_for_a_new_one() {
    let mut s = funded_grant();
    let mut i = 0;
    while s.wallets.len() < 64 {
        s = s.ensure_wallet(&format!("spam{i}.near")).unwrap();
        i += 1;
    }
    let s = s.ensure_wallet("real.near").expect("an idle slot must be freed");
    assert!(s.wallets.contains_key("real.near"));
    assert!(s.wallets.contains_key("alice"), "a grant holder is never evicted");
}

#[test]
#[should_panic(expected = "only the contract account can initialize it")]
fn a_stranger_cannot_initialize_the_contract_first() {
    ctx("attacker.test");
    Contract::new_funded("evil-token.test".parse().unwrap());
}
#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn moving_value_needs_a_wallet_confirmation() {
    let mut c = funded_contract();
    ctx("alice.test");
    c.sell_slice("g1".into(), U128(1), U128(1), "attacker.test".parse().unwrap());
}
#[test]
fn an_offer_deposit_lets_holders_draw_only_that_amount() {
    let mut c = funded_contract();
    ctx("token.test");
    c.ft_on_transfer("lena.test".parse().unwrap(), U128(40_000), r#"{"action":"offer"}"#.into());
    c.ft_on_transfer("sam.test".parse().unwrap(), U128(40_000), r#"{"action":"deposit"}"#.into());
    assert_eq!(c.lendable("lena.test".parse().unwrap()).0, 40_000);
    assert_eq!(c.lendable("sam.test".parse().unwrap()).0, 0);
}

#[test]
fn a_wallet_with_a_withdrawal_in_flight_keeps_its_slot_and_gets_restored() {
    let s = funded_grant().deposit("victim.near", 5_000).unwrap();
    let mut s = s.withdraw_reserve("victim.near", 5_000).unwrap();
    let mut i = 0;
    while s.wallets.len() < 64 {
        s = s.deposit(&format!("full{i}.near"), 1).unwrap();
        i += 1;
    }
    assert!(s.ensure_wallet("newcomer.near").is_err(), "the in-flight wallet must not be evicted");
    let s = s.withdraw_restore(SELF, "victim.near", 5_000).unwrap();
    assert_eq!(s.balance("victim.near").unwrap(), 5_000);
    assert!(s.withdrawing.is_empty());
}
#[test]
fn a_settled_withdrawal_frees_the_wallet() {
    let s = funded_grant().deposit("w.near", 5_000).unwrap();
    let s = s.withdraw_reserve("w.near", 5_000).unwrap();
    assert!(s.withdraw_settled("mallory", "w.near", 5_000).is_err());
    let s = s.withdraw_settled(SELF, "w.near", 5_000).unwrap();
    assert!(s.withdrawing.is_empty());
}
fn ctx_at(predecessor: &str, day: u64) {
    let mut b = VMContextBuilder::new();
    b.current_account_id("contract.test".parse().unwrap())
        .predecessor_account_id(predecessor.parse().unwrap())
        .block_timestamp(day * 86_400_000_000_000);
    testing_env!(b.build());
}
#[test]
fn unlock_with_no_deposit_settles_the_vested_amount() {
    let mut c = funded_contract();
    ctx("token.test");
    c.ft_on_transfer("dao.test".parse().unwrap(), U128(365_000), grant_msg());
    // Day 200: past the 90-day cliff, 200 of 365 days vested. No deposit attached, as an agent key sends it.
    ctx_at("alice.test", 200);
    c.unlock("g1".into());
    let alice: AccountId = "alice.test".parse().unwrap();
    assert_eq!(c.demo_balance(alice).0, 200_000);
    assert_eq!(c.backing().0, 365_000, "unlock moves nothing out of the contract");
}
#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn after_an_agent_unlock_a_no_deposit_withdraw_is_refused() {
    let mut c = funded_contract();
    ctx("token.test");
    c.ft_on_transfer("dao.test".parse().unwrap(), U128(365_000), grant_msg());
    ctx_at("alice.test", 200);
    c.unlock("g1".into());
    let _ = c.withdraw(U128(200_000));
}
#[test]
#[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
fn withdraw_still_needs_a_wallet_confirmation() {
    let mut c = funded_contract();
    ctx("alice.test");
    let _ = c.withdraw(U128(1));
}
