use crate::{
    engine::{Engine, TransferState},
    Contract,
};
use near_sdk::{
    test_utils::{get_created_receipts, VMContextBuilder},
    testing_env, AccountId, Gas, NearToken, PromiseResult,
};
use std::collections::{BTreeMap, HashMap};
const SELF: &str = "contract.test";
fn account(s: &str) -> AccountId {
    s.parse().unwrap()
}
fn fixture(day: u64) -> Engine {
    let mut s = Engine::new(
        SELF.into(),
        BTreeMap::from([
            ("dao.treasury".into(), 1_200_000),
            ("alice".into(), 0),
            ("lena.lender".into(), 800_000),
        ]),
        0,
    )
    .unwrap()
    .create_grant(
        "dao.treasury",
        "g1",
        "Contributor".into(),
        "alice",
        365_000,
        0,
        90,
        365,
        false,
    )
    .unwrap()
    .borrow("alice", "g1", 100_000, "lena.lender")
    .unwrap();
    s.day = day;
    s
}
fn context(predecessor: &str, day: u64) -> near_sdk::VMContext {
    VMContextBuilder::new()
        .current_account_id(account(SELF))
        .signer_account_id(account("alice"))
        .predecessor_account_id(account(predecessor))
        .block_timestamp(day * 86_400_000_000_000)
        .prepaid_gas(Gas::from_tgas(300))
        .account_balance(NearToken::from_near(100))
        .build()
}
fn callback_env(results: Vec<PromiseResult>) {
    testing_env!(
        context(SELF, 120),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        HashMap::default(),
        results
    );
}
#[test]
fn recipient_legs_cover_all_four_outcomes_and_restore_once() {
    for lender_ok in [false, true] {
        for holder_ok in [false, true] {
            let (mut s, id) = fixture(120).claim_ft("alice", "g1").unwrap();
            assert_eq!(s.transfers[&id].legs.len(), 2);
            assert_eq!(
                s.reserved_debt("g1", s.grant("g1").unwrap().lien.as_ref().unwrap().id)
                    .unwrap(),
                112_000
            );
            assert!(s.repay("alice", "g1", 1).is_err());
            for (i, success) in [lender_ok, holder_ok].into_iter().enumerate() {
                s = s.resolve_ft_leg(SELF, id, i, success).unwrap();
                let unchanged = s.clone();
                assert!(s.resolve_ft_leg(SELF, id, i, success).is_err());
                assert_eq!(s, unchanged);
            }
            assert_eq!(s.grant("g1").unwrap().pending, 0);
            assert_eq!(
                s.grant("g1").unwrap().claimed,
                if lender_ok { 112_000 } else { 0 } + if holder_ok { 8_000 } else { 0 }
            );
            assert_eq!(
                s.balance("lena.lender").unwrap(),
                700_000 + if lender_ok { 112_000 } else { 0 }
            );
            assert_eq!(
                s.balance("alice").unwrap(),
                100_000 + if holder_ok { 8_000 } else { 0 }
            );
            assert_eq!(
                s.grant("g1").unwrap().claimable(120).unwrap(),
                if lender_ok { 0 } else { 112_000 } + if holder_ok { 0 } else { 8_000 }
            );
            s.assert_invariants().unwrap();
        }
    }
}
#[test]
fn reserved_debt_cannot_be_repaid_twice() {
    let (s, id) = fixture(90).claim_ft("alice", "g1").unwrap();
    let s = s.repay("alice", "g1", 22_000).unwrap();
    assert!(s.repay("alice", "g1", 1).is_err());
    assert_eq!(s.grant("g1").unwrap().lien.as_ref().unwrap().debt, 90_000);
    let s = s.resolve_ft_leg(SELF, id, 0, true).unwrap();
    assert!(s.grant("g1").unwrap().lien.is_none());
    assert_eq!(s.balance("lena.lender").unwrap(), 812_000);
    assert!(s.resolve_ft_leg(SELF, id, 0, true).is_err());
}
#[test]
fn old_holder_leg_cannot_touch_new_lien() {
    let (s, id) = fixture(120).claim_ft("alice", "g1").unwrap();
    let old_lien = s.grant("g1").unwrap().lien.as_ref().unwrap().id;
    let s = s.resolve_ft_leg(SELF, id, 0, true).unwrap();
    let s = s.borrow("alice", "g1", 5_000, "lena.lender").unwrap();
    let new_lien = s.grant("g1").unwrap().lien.as_ref().unwrap().clone();
    assert_ne!(old_lien, new_lien.id);
    let lender_before = s.balance("lena.lender").unwrap();
    let s = s.resolve_ft_leg(SELF, id, 1, true).unwrap();
    assert_eq!(
        s.grant("g1").unwrap().lien.as_ref().unwrap().debt,
        new_lien.debt
    );
    assert_eq!(s.balance("lena.lender").unwrap(), lender_before);
}
#[test]
fn cancellation_pending_shortfall_remains_correct_on_success_and_failure() {
    for success in [false, true] {
        let (mut s, id) = fixture(100).claim_ft("alice", "g1").unwrap();
        // Legacy cancellable-with-lien fixture only, as in the retained Python PoC.
        s.grants.get_mut("g1").unwrap().cancellable = true;
        let s = s.cancel("dao.treasury", "g1").unwrap();
        assert_eq!(
            s.grant("g1").unwrap().lien.as_ref().unwrap().shortfall,
            12_000
        );
        let s = s.resolve_ft_leg(SELF, id, 0, success).unwrap();
        assert_eq!(
            s.grant("g1").unwrap().lien.as_ref().unwrap().shortfall,
            12_000
        );
        assert_eq!(
            s.grant("g1").unwrap().escrow,
            if success { 0 } else { 100_000 }
        );
        s.assert_invariants().unwrap();
    }
}
#[test]
fn invalid_leg_or_actor_does_not_mutate() {
    let (s, id) = fixture(120).claim_ft("alice", "g1").unwrap();
    let before = s.clone();
    assert!(s.resolve_ft_leg("alice", id, 0, true).is_err());
    assert!(s.resolve_ft_leg(SELF, id, 2, true).is_err());
    assert_eq!(s, before);
}
#[test]
fn public_demo_initializer_keeps_ft_disabled() {
    testing_env!(context("alice", 0));
    let c = Contract::new_demo(BTreeMap::from([(
        account("alice"),
        near_sdk::json_types::U128(1),
    )]));
    assert!(c.token_contract.is_none());
    assert!(c.accounting_mode().contains("disabled"));
}
#[test]
fn mocked_receipts_have_exact_recipients_amounts_deposit_and_joined_callback() {
    testing_env!(context("alice", 120));
    let mut c = Contract {
        engine: fixture(120),
        token_contract: Some(account("token.test")),
        reconciliations: BTreeMap::new(),
        funded: false,
    };
    drop(c.claim("g1".into()));
    let receipts = get_created_receipts();
    let mut transfers = BTreeMap::new();
    let mut callbacks = 0;
    for receipt in receipts {
        for action in receipt.actions {
            if let near_sdk::mock::MockAction::FunctionCallWeight {
                method_name,
                args,
                attached_deposit,
                prepaid_gas,
                ..
            } = action
            {
                let args: near_sdk::serde_json::Value =
                    near_sdk::serde_json::from_slice(&args).unwrap();
                if method_name == b"ft_transfer" {
                    assert_eq!(receipt.receiver_id, account("token.test"));
                    assert_eq!(attached_deposit.as_yoctonear(), 1);
                    assert_eq!(prepaid_gas, Gas::from_tgas(20));
                    transfers.insert(
                        args["receiver_id"].as_str().unwrap().to_string(),
                        args["amount"].as_str().unwrap().to_string(),
                    );
                } else if method_name == b"on_claim_resolved" {
                    assert_eq!(receipt.receiver_id, account(SELF));
                    assert_eq!(receipt.receipt_indices.len(), 2);
                    assert_eq!(args["transfer_id"], 1);
                    assert_eq!(prepaid_gas, Gas::from_tgas(60));
                    callbacks += 1;
                }
            }
        }
    }
    assert_eq!(
        transfers,
        BTreeMap::from([
            ("lena.lender".to_string(), "112000".to_string()),
            ("alice".to_string(), "8000".to_string())
        ])
    );
    assert_eq!(callbacks, 1);
}
#[test]
fn mocked_callback_all_four_results_and_replay() {
    for lender_ok in [false, true] {
        for holder_ok in [false, true] {
            testing_env!(context("alice", 120));
            let mut c = Contract {
                engine: fixture(120),
                token_contract: Some(account("token.test")),
                reconciliations: BTreeMap::new(),
                funded: false,
            };
            drop(c.claim("g1".into()));
            callback_env(
                [lender_ok, holder_ok]
                    .into_iter()
                    .map(|ok| {
                        if ok {
                            PromiseResult::Successful(b"null".to_vec())
                        } else {
                            PromiseResult::Failed
                        }
                    })
                    .collect(),
            );
            assert_eq!(c.on_claim_resolved(1), lender_ok && holder_ok);
            assert_eq!(c.engine.grant("g1").unwrap().pending, 0);
            assert_ne!(c.engine.transfers[&1].state, TransferState::Pending);
            let before = c.engine.clone();
            assert!(!c.on_claim_resolved(1));
            assert_eq!(c.engine, before);
        }
    }
}

#[test]
fn callback_engine_error_records_outcomes_without_resending_or_false_restore() {
    testing_env!(context("alice", 120));
    let mut c = Contract {
        engine: fixture(120),
        token_contract: Some(account("token.test")),
        reconciliations: BTreeMap::new(),
        funded: false,
    };
    drop(c.claim("g1".into()));
    // Real negative control: corrupt the actual reserved generation after scheduling.
    c.engine
        .grants
        .get_mut("g1")
        .unwrap()
        .lien
        .as_mut()
        .unwrap()
        .id += 1;
    callback_env(vec![
        PromiseResult::Successful(b"null".to_vec()),
        PromiseResult::Successful(b"null".to_vec()),
    ]);
    assert!(!c.on_claim_resolved(1));
    let r = c.needs_reconcile();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].observed_outcomes, vec![true, true]);
    assert!(r[0].reason.contains("generation"));
    assert_eq!(c.engine.grant("g1").unwrap().pending, 120_000);
    assert_eq!(c.engine.balance("lena.lender").unwrap(), 700_000);
    assert_eq!(c.engine.transfers[&1].state, TransferState::NeedsReconcile);
    assert!(c.engine.repay("alice", "g1", 1).is_err());
    assert!(c.engine.borrow("alice", "g1", 1, "lena.lender").is_err());
    assert!(c.engine.claim_ft("alice", "g1").is_err());
    let before = c.engine.clone();
    assert!(!c.on_claim_resolved(1));
    assert_eq!(c.engine, before);
}
#[test]
fn unexpected_callback_count_is_visible_and_preserves_pending() {
    testing_env!(context("alice", 120));
    let mut c = Contract {
        engine: fixture(120),
        token_contract: Some(account("token.test")),
        reconciliations: BTreeMap::new(),
        funded: false,
    };
    drop(c.claim("g1".into()));
    callback_env(vec![PromiseResult::Failed]);
    assert!(!c.on_claim_resolved(1));
    assert_eq!(
        c.needs_reconcile()[0].reason,
        "Unexpected callback result count"
    );
    assert_eq!(c.engine.grant("g1").unwrap().pending, 120_000);
    c.engine.assert_invariants().unwrap();
}
#[test]
fn unauthorized_callback_is_noop() {
    testing_env!(context("alice", 120));
    let (s, _) = fixture(120).claim_ft("alice", "g1").unwrap();
    let mut c = Contract {
        engine: s,
        token_contract: Some(account("token.test")),
        reconciliations: BTreeMap::new(),
        funded: false,
    };
    let before = c.engine.clone();
    assert!(!c.on_claim_resolved(1));
    assert_eq!(c.engine, before);
    assert!(c.needs_reconcile().is_empty());
}

#[test]
fn state_growth_caps_reject_before_changing_original() {
    let many = (0..65).map(|i| (format!("account-{i}.test"), 0)).collect();
    assert!(Engine::new(SELF.into(), many, 0).is_err());
    let mut s = fixture(120);
    for i in 1..32 {
        s = s
            .create_grant(
                "dao.treasury",
                &format!("extra-{i}"),
                "Extra".into(),
                "alice",
                1,
                0,
                0,
                1,
                false,
            )
            .unwrap();
    }
    let before = s.clone();
    assert!(s
        .create_grant(
            "dao.treasury",
            "too-many",
            "No".into(),
            "alice",
            1,
            0,
            0,
            1,
            false
        )
        .is_err());
    assert_eq!(s, before);
    for _ in 0..128 {
        let (next, id) = s.claim_ft("alice", "g1").unwrap();
        s = next
            .resolve_ft_leg(SELF, id, 0, false)
            .unwrap()
            .resolve_ft_leg(SELF, id, 1, false)
            .unwrap();
    }
    let before = s.clone();
    assert!(s.claim_ft("alice", "g1").is_err());
    assert_eq!(s, before);
}
#[test]
fn bounded_large_fixture_callback_completes_in_native_mock_only() {
    let mut s = fixture(120);
    for i in 0..61 {
        s.wallets.insert(format!("extra-wallet-{i}.test"), 0);
    }
    for i in 1..32 {
        s = s
            .create_grant(
                "dao.treasury",
                &format!("extra-{i}"),
                "Extra".into(),
                "alice",
                1,
                0,
                0,
                1,
                false,
            )
            .unwrap();
    }
    for _ in 0..127 {
        let (next, id) = s.claim_ft("alice", "g1").unwrap();
        s = next
            .resolve_ft_leg(SELF, id, 0, false)
            .unwrap()
            .resolve_ft_leg(SELF, id, 1, false)
            .unwrap();
    }
    let mut closed = s.grant("g1").unwrap().lien.as_ref().unwrap().clone();
    closed.debt = 0;
    closed.shortfall = 0;
    closed.closed_day = Some(119);
    for (id, grant) in &mut s.grants {
        grant.history = vec![closed.clone(); if id == "g1" { 31 } else { 32 }];
    }
    s.assert_invariants().unwrap();
    testing_env!(context("alice", 120));
    let mut c = Contract {
        engine: s,
        token_contract: Some(account("token.test")),
        reconciliations: BTreeMap::new(),
        funded: false,
    };
    drop(c.claim("g1".into()));
    assert_eq!(c.engine.transfers.len(), 128);
    let bytes = near_sdk::borsh::to_vec(&c).unwrap().len();
    assert!(
        bytes <= 512 * 1024,
        "bounded serialized fixture exceeds metadata allowance"
    );
    callback_env(vec![
        PromiseResult::Successful(b"null".to_vec()),
        PromiseResult::Successful(b"null".to_vec()),
    ]);
    assert!(c.on_claim_resolved(128));
    assert_eq!(c.engine.grant("g1").unwrap().pending, 0);
    assert!(c.needs_reconcile().is_empty());
    c.engine.assert_invariants().unwrap();
    // Native mock does not charge WASM instruction gas. No gas-sizing inference.
}
