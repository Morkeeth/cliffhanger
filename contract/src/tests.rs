use crate::engine::{Engine, TransferState};
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
        .create_grant(
            "dao.treasury",
            "g1",
            "Core contributor grant".into(),
            "alice",
            365_000,
            0,
            90,
            365,
            false,
        )
        .unwrap();
    s.create_grant(
        "dao.treasury",
        "g2",
        "Advisor grant".into(),
        "alice",
        120_000,
        0,
        30,
        180,
        true,
    )
    .unwrap()
}
fn advance(mut s: Engine, days: u64) -> Engine {
    s.day += days;
    s.assert_invariants().unwrap();
    s
}
fn resolve(s: Engine, tid: u64, success: bool) -> Engine {
    s.resolve(SELF, tid, success).unwrap()
}
#[test]
fn entry_shows_grants_and_options() {
    let s = initial();
    assert_eq!(s.grants.len(), 2);
    assert_eq!(s.grant("g1").unwrap().vested(0).unwrap(), 0);
    assert_eq!(s.grant("g1").unwrap().unvested(0).unwrap(), 365_000);
    assert_eq!(s.quote("g1").unwrap().max_advance, 146_000);
    assert_eq!(s.quote("g2").unwrap().ltv_pct, 0);
    assert_eq!(s.quote("g2").unwrap().max_advance, 0);
}
#[test]
fn full_self_repaying_path() {
    let s = initial()
        .borrow("alice", "g1", 100_000, "lena.lender")
        .unwrap();
    assert_eq!(s.balance("alice").unwrap(), 100_000);
    assert_eq!(s.grant("g1").unwrap().lien.as_ref().unwrap().debt, 112_000);
    let s = advance(s, 120);
    let (s, tid) = s.claim("alice", "g1").unwrap();
    let s = resolve(s, tid, true);
    assert_eq!(s.balance("lena.lender").unwrap(), 812_000);
    assert!(s.grant("g1").unwrap().lien.is_none());
    assert_eq!(s.balance("alice").unwrap(), 108_000);
}
#[test]
fn no_double_pledge_and_limit() {
    let s = initial();
    assert!(s.borrow("alice", "g1", 146_001, "lena.lender").is_err());
    let s = s.borrow("alice", "g1", 1_000, "lena.lender").unwrap();
    assert!(s.borrow("alice", "g1", 1_000, "lena.lender").is_err());
}
#[test]
fn wrong_actor_rejected_and_state_unchanged() {
    let s = initial();
    let before = s.clone();
    assert!(s.borrow("lena.lender", "g1", 10, "lena.lender").is_err());
    assert!(s.cancel("alice", "g2").is_err());
    assert!(s.cancel("dao.treasury", "g1").is_err());
    assert_eq!(s, before);
}
#[test]
fn failed_transfer_restores_once() {
    let s = advance(initial(), 100);
    let (s, t1) = s.claim("alice", "g1").unwrap();
    assert!(s.claim("alice", "g1").is_err());
    let s = resolve(s, t1, false);
    assert_eq!(s.grant("g1").unwrap().escrow, 365_000);
    assert!(s.resolve(SELF, t1, false).is_err());
    assert_eq!(s.transfers[&t1].state, TransferState::FailedRestored);
    let (s, t2) = s.claim("alice", "g1").unwrap();
    let s = resolve(s, t2, true);
    assert_eq!(s.balance("alice").unwrap(), 100_000);
}
#[test]
fn early_repay_releases_lien() {
    let s = initial()
        .borrow("alice", "g1", 50_000, "lena.lender")
        .unwrap();
    let debt = s.grant("g1").unwrap().lien.as_ref().unwrap().debt;
    let s = s.repay("alice", "g1", 50_000).unwrap();
    assert!(s.repay("alice", "g1", debt - 50_000).is_err());
    let s = advance(s, 200);
    let (s, tid) = s.claim("alice", "g1").unwrap();
    let s = resolve(s, tid, true);
    assert!(s.grant("g1").unwrap().lien.is_none());
    assert_eq!(s.grant("g1").unwrap().history.len(), 1);
}
#[test]
fn repay_while_claim_pending_never_overpays_lender() {
    let s = initial()
        .borrow("alice", "g1", 10_000, "lena.lender")
        .unwrap();
    let s = advance(s, 100);
    let debt = s.grant("g1").unwrap().lien.as_ref().unwrap().debt;
    let (s, tid) = s.claim("alice", "g1").unwrap();
    let s = s.repay("alice", "g1", 10_000).unwrap();
    let s = resolve(s, tid, true);
    assert_eq!(s.balance("lena.lender").unwrap(), 800_000 - 10_000 + debt);
    assert!(s.grant("g1").unwrap().lien.is_none());
}
#[test]
fn cancellation_returns_unvested_and_shows_shortfall() {
    let mut s = initial(); // Same legacy-terms fixture as Python; no public contract setter.
    s.grants.get_mut("g2").unwrap().cancellable = false;
    let mut s = s.borrow("alice", "g2", 40_000, "lena.lender").unwrap();
    s.grants.get_mut("g2").unwrap().cancellable = true;
    let s = advance(s, 60).cancel("dao.treasury", "g2").unwrap();
    let g = s.grant("g2").unwrap();
    assert_eq!(g.escrow, 40_000);
    let l = g.lien.as_ref().unwrap();
    assert_eq!(l.shortfall, l.debt - 40_000);
    assert!(l.shortfall > 0);
}
#[test]
fn full_repay_while_claim_pending_does_not_pay_lender_twice() {
    let s = initial()
        .borrow("alice", "g1", 10_000, "lena.lender")
        .unwrap();
    let s = advance(s, 100);
    let (s, t1) = s.claim("alice", "g2").unwrap();
    let s = resolve(s, t1, true);
    let (s, t2) = s.claim("alice", "g1").unwrap();
    let debt = s.grant("g1").unwrap().lien.as_ref().unwrap().debt;
    let s = s.repay("alice", "g1", debt).unwrap();
    let before = s.balance("lena.lender").unwrap();
    assert!(s.grant("g1").unwrap().lien.is_none());
    let s = resolve(s, t2, true);
    assert_eq!(s.balance("lena.lender").unwrap(), before);
    assert_eq!(before, 800_000 - 10_000 + debt);
}
#[test]
fn old_pending_claim_never_repays_a_new_lien() {
    let s = initial()
        .borrow("alice", "g1", 10_000, "lena.lender")
        .unwrap();
    let s = advance(s, 100);
    let (s, t1) = s.claim("alice", "g2").unwrap();
    let s = resolve(s, t1, true);
    let (s, t2) = s.claim("alice", "g1").unwrap();
    let debt = s.grant("g1").unwrap().lien.as_ref().unwrap().debt;
    let s = s.repay("alice", "g1", debt).unwrap();
    let s = s.borrow("alice", "g1", 5_000, "lena.lender").unwrap();
    let before = s.balance("lena.lender").unwrap();
    let new_debt = s.grant("g1").unwrap().lien.as_ref().unwrap().debt;
    let s = resolve(s, t2, true);
    assert_eq!(s.balance("lena.lender").unwrap(), before);
    assert_eq!(s.grant("g1").unwrap().lien.as_ref().unwrap().debt, new_debt);
}
#[test]
fn wrong_callback_cannot_modify_state() {
    let s = advance(initial(), 100);
    let (s, tid) = s.claim("alice", "g1").unwrap();
    let before = s.clone();
    assert!(s.resolve("alice", tid, true).is_err());
    assert_eq!(s, before);
}
#[test]
fn invalid_grant_or_overflow_keeps_original_state() {
    let s = initial();
    let before = s.clone();
    assert!(s
        .create_grant(
            "dao.treasury",
            "bad",
            "Bad".into(),
            "alice",
            1,
            0,
            2,
            1,
            false
        )
        .is_err());
    assert!(s
        .create_grant(
            "dao.treasury",
            "bad",
            "Bad".into(),
            "alice",
            1,
            u64::MAX,
            0,
            1,
            false
        )
        .is_err());
    assert_eq!(s, before);
    assert!(Engine::new(
        SELF.into(),
        BTreeMap::from([("a".into(), u128::MAX), ("b".into(), 1)]),
        0
    )
    .is_err());
}
