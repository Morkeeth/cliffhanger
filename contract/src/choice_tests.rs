//! Choices 1 and 3 beside the loan. Mirrors projects/near/poc/test_choices.py.
use crate::engine::{Engine, STAKE_BP};
use std::collections::BTreeMap;
const SELF: &str = "contract.test";
fn initial() -> Engine {
    let wallets = BTreeMap::from([
        ("dao.treasury".into(), 1_200_000),
        ("alice".into(), 0),
        ("lena.lender".into(), 800_000),
    ]);
    let s = Engine::new(SELF.into(), wallets, 0).unwrap();
    let s = s
        .create_grant("dao.treasury", "g1", "Core contributor grant".into(), "alice", 365_000, 0, 90, 365, false)
        .unwrap();
    s.create_grant("dao.treasury", "g2", "Advisor grant".into(), "alice", 120_000, 0, 30, 180, true)
        .unwrap()
}
fn at(mut s: Engine, day: u64) -> Engine {
    s.advance_to(day).unwrap();
    s
}
fn settle(s: Engine, actor: &str, id: &str) -> Engine {
    let (s, tid) = s.claim_ft(actor, id).unwrap();
    let n = s.transfers[&tid].legs.len();
    (0..n).fold(s, |s, i| s.resolve_ft_leg(SELF, tid, i, true).unwrap())
}
fn sold(s: Engine, slice: u128, price: u128) -> Engine {
    s.sell_slice("alice", "g1", slice, price, "lena.lender").unwrap()
}

#[test]
fn buyer_pays_now_and_is_paid_first_until_the_slice_is_delivered() {
    let s = sold(initial(), 50_000, 30_000);
    assert_eq!(s.balance("alice").unwrap(), 30_000);
    assert_eq!(s.balance("lena.lender").unwrap(), 770_000);
    let s = settle(at(s, 100), "alice", "g1");
    assert_eq!(s.balance("lena.lender").unwrap(), 770_000 + 50_000);
    assert_eq!(s.balance("alice").unwrap(), 30_000 + 50_000);
    let g = s.grant("g1").unwrap();
    assert!(g.sale.is_none());
    assert_eq!(g.sale_history.last().unwrap().delivered, 50_000);
}
#[test]
fn partial_unlocks_deliver_the_slice_over_time_and_buyer_can_claim() {
    let s = settle(at(sold(initial(), 120_000, 70_000), 100), "alice", "g1");
    assert_eq!(s.grant("g1").unwrap().sale.as_ref().unwrap().delivered, 100_000);
    assert_eq!(s.balance("alice").unwrap(), 70_000);
    let s = settle(at(s, 130), "lena.lender", "g1");
    assert!(s.grant("g1").unwrap().sale.is_none());
    assert_eq!(s.balance("alice").unwrap(), 80_000);
}
#[test]
fn failed_leg_restores_and_never_double_delivers() {
    let s = at(sold(initial(), 50_000, 30_000), 100);
    let (s, tid) = s.claim_ft("alice", "g1").unwrap();
    assert_eq!(s.reserved_debt("g1", 1).unwrap(), 50_000);
    let s = s.resolve_ft_leg(SELF, tid, 0, false).unwrap();
    let s = s.resolve_ft_leg(SELF, tid, 1, true).unwrap();
    assert_eq!(s.grant("g1").unwrap().sale.as_ref().unwrap().delivered, 0);
    assert_eq!(s.reserved_debt("g1", 1).unwrap(), 0);
    let s = settle(s, "alice", "g1");
    assert_eq!(s.balance("lena.lender").unwrap(), 770_000 + 50_000);
}
#[test]
fn one_commitment_per_grant() {
    let s = sold(initial(), 50_000, 30_000);
    assert!(s.borrow("alice", "g1", 1_000, "lena.lender").is_err());
    assert!(s.stake("alice", "g1").is_err());
    assert!(s.sell_slice("alice", "g1", 10, 5, "lena.lender").is_err());
    assert_eq!(s.quote("g1").unwrap().max_advance, 0);
    let s = initial().stake("alice", "g1").unwrap();
    assert!(s.borrow("alice", "g1", 1_000, "lena.lender").is_err());
    assert!(s.sell_slice("alice", "g1", 10, 5, "lena.lender").is_err());
}
#[test]
fn sale_limits_and_wrong_callers_leave_state_unchanged() {
    let s = initial();
    assert!(s.sell_slice("alice", "g2", 10, 5, "lena.lender").is_err());
    assert!(s.sell_slice("alice", "g1", 365_001, 5, "lena.lender").is_err());
    assert!(s.sell_slice("alice", "g1", 100, 101, "lena.lender").is_err());
    assert!(s.sell_slice("lena.lender", "g1", 100, 50, "dao.treasury").is_err());
    assert!(s.sell_slice("alice", "g1", 100, 50, "alice").is_err());
    assert_eq!(s, initial());
}
#[test]
fn cancel_after_sale_shows_shortfall() {
    let mut s = initial();
    s.grants.get_mut("g2").unwrap().cancellable = false;
    let mut s = s.sell_slice("alice", "g2", 80_000, 40_000, "lena.lender").unwrap();
    s.grants.get_mut("g2").unwrap().cancellable = true;
    let s = at(s, 60).cancel("dao.treasury", "g2").unwrap();
    assert_eq!(s.grant("g2").unwrap().sale.as_ref().unwrap().shortfall, 40_000);
}
#[test]
fn rewards_accrue_on_escrow_and_mint_on_collect() {
    let s = at(initial().stake("alice", "g1").unwrap(), 365);
    let expected = 365_000 * STAKE_BP / 10_000;
    assert_eq!(s.grant("g1").unwrap().stake.as_ref().unwrap().unpaid, expected);
    let supply = s.supply;
    let s = s.collect_rewards("alice", "g1").unwrap();
    assert_eq!(s.balance("alice").unwrap(), expected);
    assert_eq!(s.supply, supply + expected);
    assert!(s.collect_rewards("alice", "g1").is_err());
}
#[test]
fn holder_keeps_every_unlock_while_staked() {
    let s = settle(at(initial().stake("alice", "g1").unwrap(), 100), "alice", "g1");
    assert_eq!(s.balance("alice").unwrap(), 100_000);
    let s = at(s, 110);
    assert_eq!(
        s.grant("g1").unwrap().stake.as_ref().unwrap().unpaid,
        365_000 * STAKE_BP * 100 / (10_000 * 365) + 265_000 * STAKE_BP * 10 / (10_000 * 365)
    );
}
#[test]
fn unstake_needs_rewards_collected_then_frees_the_grant() {
    let s = at(initial().stake("alice", "g1").unwrap(), 30);
    assert!(s.unstake("alice", "g1").is_err());
    let s = s.collect_rewards("alice", "g1").unwrap().unstake("alice", "g1").unwrap();
    assert!(s.quote("g1").unwrap().max_advance > 0);
    assert_eq!(s.grant("g1").unwrap().stake_history.len(), 1);
}
#[test]
fn cancel_ends_stake_and_pays_earned_rewards() {
    let s = at(initial().stake("alice", "g2").unwrap(), 60);
    let earned = s.grant("g2").unwrap().stake.as_ref().unwrap().unpaid;
    assert!(earned > 0);
    let s = s.cancel("dao.treasury", "g2").unwrap();
    assert!(s.grant("g2").unwrap().stake.is_none());
    assert_eq!(s.balance("alice").unwrap(), earned);
}
