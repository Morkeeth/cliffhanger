//! Local slice-2 transfer adapter. Public demo initialization leaves it disabled.
//! Funding and loan wallets remain synthetic; no deployment or asset-safety claim.
mod engine;
use engine::{Amount, Engine, Grant, Lien, Sale, Stake};
use near_sdk::json_types::U128;
use near_sdk::serde::Serialize;
use near_sdk::{
    env, ext_contract, near, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseResult,
};
use std::collections::BTreeMap;

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    engine: Engine,
    // No public initializer or setter enables this until real funding is implemented.
    token_contract: Option<AccountId>,
    reconciliations: BTreeMap<u64, ReconcileRecord>,
    /// Funded mode: every balance and escrow is backed by real NEP-141 tokens held by this contract.
    funded: bool,
}

#[derive(near_sdk::serde::Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct FundMsg {
    action: String,
    grant_id: Option<String>,
    title: Option<String>,
    holder: Option<AccountId>,
    start: Option<u64>,
    cliff: Option<u64>,
    duration: Option<u64>,
    cancellable: Option<bool>,
}

#[derive(Clone, near_sdk::borsh::BorshDeserialize, near_sdk::borsh::BorshSerialize, Serialize)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct ReconcileRecord {
    transfer_id: u64,
    reason: String,
    observed_outcomes: Vec<bool>,
    at_day: u64,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct LienView {
    id: u64,
    lender: String,
    principal: U128,
    fee: U128,
    debt: U128,
    repaid_by_unlocks: U128,
    repaid_by_holder: U128,
    shortfall: U128,
    reserved: U128,
}
impl From<&Lien> for LienView {
    fn from(l: &Lien) -> Self {
        Self {
            id: l.id,
            lender: l.lender.clone(),
            principal: U128(l.principal),
            fee: U128(l.fee),
            debt: U128(l.debt),
            repaid_by_unlocks: U128(l.repaid_by_unlocks),
            repaid_by_holder: U128(l.repaid_by_holder),
            shortfall: U128(l.shortfall),
            reserved: U128(0),
        }
    }
}
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleView {
    id: u64,
    buyer: String,
    slice: U128,
    price: U128,
    delivered: U128,
    shortfall: U128,
    reserved: U128,
}
impl From<&Sale> for SaleView {
    fn from(s: &Sale) -> Self {
        Self {
            id: s.id,
            buyer: s.buyer.clone(),
            slice: U128(s.slice),
            price: U128(s.price),
            delivered: U128(s.delivered),
            shortfall: U128(s.shortfall),
            reserved: U128(0),
        }
    }
}
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct StakeView {
    id: u64,
    yield_bp: U128,
    unpaid: U128,
    paid: U128,
}
impl From<&Stake> for StakeView {
    fn from(s: &Stake) -> Self {
        Self {
            id: s.id,
            yield_bp: U128(s.yield_bp),
            unpaid: U128(s.unpaid),
            paid: U128(s.paid),
        }
    }
}
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct GrantView {
    id: String,
    title: String,
    issuer: String,
    holder: String,
    total: U128,
    vested: U128,
    claimed: U128,
    claimable: U128,
    unvested: U128,
    escrow: U128,
    pending: U128,
    returned_to_issuer: U128,
    cancellable: bool,
    cancelled_at: Option<u64>,
    next_unlock_day: Option<u64>,
    lien: Option<LienView>,
    sale: Option<SaleView>,
    stake: Option<StakeView>,
}
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct QuoteView {
    ltv_pct: u8,
    max_advance: U128,
    term_days: u64,
    fee: Option<U128>,
    within_limit: Option<bool>,
    why_not: Option<String>,
}
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct OptionsView {
    grant: GrantView,
    can_claim: bool,
    borrow: QuoteView,
    can_repay: bool,
    can_sell: bool,
    max_slice: U128,
    can_stake: bool,
    can_collect: bool,
}

fn checked<T>(value: engine::Result<T>) -> T {
    value.unwrap_or_else(|error| env::panic_str(&error))
}
fn day() -> u64 {
    env::block_timestamp_ms() / 86_400_000
}
impl Contract {
    fn mark_reconcile(&mut self, transfer_id: u64, reason: &str, outcomes: Vec<bool>) {
        if let Some(t) = self.engine.transfers.get_mut(&transfer_id) {
            for leg in &mut t.legs {
                if leg.state == engine::TransferState::Pending {
                    leg.state = engine::TransferState::NeedsReconcile;
                }
            }
            t.state = engine::TransferState::NeedsReconcile;
            let record = ReconcileRecord {
                transfer_id,
                reason: reason.chars().take(160).collect(),
                observed_outcomes: outcomes,
                at_day: day(),
            };
            env::log_str(
                &near_sdk::serde_json::to_string(&record)
                    .unwrap_or_else(|_| "Claim needs reconciliation".into()),
            );
            self.reconciliations.insert(transfer_id, record);
        }
    }
    fn current(&self) -> Engine {
        let mut model = self.engine.clone();
        if day() < model.day {
            env::panic_str("block day moved backwards");
        }
        checked(model.advance_to(day()));
        model
    }
    fn grant_view(model: &Engine, id: &str, g: &Grant) -> GrantView {
        GrantView {
            id: id.into(),
            title: g.title.clone(),
            issuer: g.issuer.clone(),
            holder: g.holder.clone(),
            total: U128(g.total),
            vested: U128(checked(g.vested(model.day))),
            claimed: U128(g.claimed),
            claimable: U128(checked(g.claimable(model.day))),
            unvested: U128(checked(g.unvested(model.day))),
            escrow: U128(g.escrow),
            pending: U128(g.pending),
            returned_to_issuer: U128(g.returned),
            cancellable: g.cancellable,
            cancelled_at: g.cancelled_at,
            next_unlock_day: if model.day < g.start + g.cliff {
                Some(g.start + g.cliff)
            } else {
                None
            },
            lien: g.lien.as_ref().map(|l| {
                let mut view = LienView::from(l);
                view.reserved = U128(checked(model.reserved_debt(id, l.id)));
                view
            }),
            sale: g.sale.as_ref().map(|s| {
                let mut view = SaleView::from(s);
                view.reserved = U128(checked(model.reserved_debt(id, s.id)));
                view
            }),
            stake: g.stake.as_ref().map(StakeView::from),
        }
    }
    fn quote_view(model: &Engine, id: &str, amount: Option<Amount>) -> QuoteView {
        let g = checked(model.grant(id));
        let q = checked(model.quote(id));
        QuoteView {
            ltv_pct: q.ltv_pct,
            max_advance: U128(q.max_advance),
            term_days: q.term_days,
            fee: amount.map(|a| U128(checked(q.fee_for(a)))),
            within_limit: amount.map(|a| a > 0 && a <= q.max_advance),
            why_not: if q.max_advance > 0 {
                None
            } else {
                Some(
                    if g.committed() {
                        "already committed"
                    } else if g.cancellable {
                        "cancellable grants cannot back a loan"
                    } else {
                        "nothing left unvested"
                    }
                    .into(),
                )
            },
        }
    }
}
#[allow(dead_code)]
#[ext_contract(ext_ft)]
trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
}
#[near]
impl Contract {
    /// Synthetic balances only. Not an asset deposit or a token issuance endpoint.
    #[init]
    pub fn new_demo(demo_balances: BTreeMap<AccountId, U128>) -> Self {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(), "only the contract account can initialize it, in the deploy transaction");
        let wallets = demo_balances
            .into_iter()
            .map(|(a, v)| (a.to_string(), v.0))
            .collect();
        Self {
            engine: checked(Engine::new(
                env::current_account_id().to_string(),
                wallets,
                day(),
            )),
            token_contract: None,
            reconciliations: BTreeMap::new(),
            funded: false,
        }
    }
    /// Funded mode. Tokens enter only through `ft_on_transfer` from `token_contract` and leave only
    /// through `withdraw`, so the contract's token balance always equals `backing()`.
    #[init]
    pub fn new_funded(token_contract: AccountId) -> Self {
        assert_eq!(env::predecessor_account_id(), env::current_account_id(), "only the contract account can initialize it, in the deploy transaction");
        Self {
            engine: checked(Engine::new(
                env::current_account_id().to_string(),
                BTreeMap::new(),
                day(),
            )),
            token_contract: Some(token_contract),
            reconciliations: BTreeMap::new(),
            funded: true,
        }
    }
    /// NEP-141 receiver. msg is {"action":"deposit"} or
    /// {"action":"create_grant","grant_id":..,"title":..,"holder":..,"start":..,"cliff":..,"duration":..,"cancellable":..}.
    /// Any failure panics, and the token contract refunds the full amount.
    pub fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> U128 {
        assert!(self.funded, "deposits need the funded contract");
        assert_eq!(
            Some(env::predecessor_account_id()),
            self.token_contract,
            "only the configured token can deposit"
        );
        let m: FundMsg = near_sdk::serde_json::from_str(&msg)
            .unwrap_or_else(|_| env::panic_str("msg must be deposit or create_grant JSON"));
        let mut model = checked(self.current().deposit(sender_id.as_str(), amount.0));
        if m.action == "create_grant" {
            let holder = m.holder.unwrap_or_else(|| env::panic_str("holder required"));
            model = checked(model.ensure_wallet(holder.as_str()));
            model = checked(model.create_grant(
                sender_id.as_str(),
                &m.grant_id.unwrap_or_else(|| env::panic_str("grant_id required")),
                m.title.unwrap_or_default(),
                holder.as_str(),
                amount.0,
                m.start.unwrap_or_else(day),
                m.cliff.unwrap_or(0),
                m.duration.unwrap_or_else(|| env::panic_str("duration required")),
                m.cancellable.unwrap_or(false),
            ));
        } else if m.action == "offer" {
            // Deposit and agree that holders may draw this much as a loan or a slice payment.
            let offered = model.lendable_of(sender_id.as_str()).checked_add(amount.0).unwrap_or_else(|| env::panic_str("offer overflow"));
            model = checked(model.set_lendable(sender_id.as_str(), offered));
        } else if m.action != "deposit" {
            env::panic_str("unknown action");
        }
        self.engine = model;
        U128(0)
    }
    /// Funded mode: move unlocked tokens into Cliffhanger balances. The lender or buyer is paid first.
    pub fn unlock(&mut self, grant_id: String) {
        assert!(self.funded, "unlock needs the funded contract; the demo uses claim");
        self.engine = checked(
            self.current()
                .unlock_internal(env::predecessor_account_id().as_str(), &grant_id),
        );
    }
    /// Funded mode: send real tokens from your Cliffhanger balance to your wallet.
    #[payable]
    pub fn withdraw(&mut self, amount: U128) -> Promise {
        near_sdk::assert_one_yocto();
        assert!(self.funded, "withdraw needs the funded contract");
        assert!(
            env::prepaid_gas().as_gas() >= Gas::from_tgas(60).as_gas(),
            "60 Tgas required for the transfer and callback"
        );
        let who = env::predecessor_account_id();
        self.engine = checked(self.current().withdraw_reserve(who.as_str(), amount.0));
        let token = self.token_contract.clone().unwrap();
        ext_ft::ext(token)
            .with_attached_deposit(NearToken::from_yoctonear(1))
            .with_static_gas(Gas::from_tgas(20))
            .ft_transfer(who.clone(), amount, Some("cliffhanger withdraw".into()))
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(20))
                    .on_withdraw_resolved(who, amount),
            )
    }
    #[private]
    pub fn on_withdraw_resolved(&mut self, account: AccountId, amount: U128) -> bool {
        let ok = matches!(env::promise_result(0), PromiseResult::Successful(_));
        if !ok {
            self.engine = checked(self.engine.withdraw_restore(
                env::current_account_id().as_str(),
                account.as_str(),
                amount.0,
            ));
        }
        ok
    }
    /// Agree that holders may draw up to `amount` of your Cliffhanger balance as a loan or a slice payment.
    /// Nobody can lend or buy with your balance unless you set this. 0 withdraws the offer.
    #[payable]
    pub fn set_lendable(&mut self, amount: U128) {
        near_sdk::assert_one_yocto();
        self.engine = checked(
            self.current()
                .set_lendable(env::predecessor_account_id().as_str(), amount.0),
        );
    }
    pub fn lendable(&self, account_id: AccountId) -> U128 {
        U128(self.engine.lendable_of(account_id.as_str()))
    }
    /// Tokens this contract must hold: custodial balances plus escrow plus in-flight claims.
    pub fn backing(&self) -> U128 {
        U128(self.engine.supply)
    }
    #[payable]
    pub fn create_grant(
        &mut self,
        grant_id: String,
        title: String,
        holder: AccountId,
        total: U128,
        start: u64,
        cliff: u64,
        duration: u64,
        cancellable: bool,
    ) {
        near_sdk::assert_one_yocto();
        self.engine = checked(self.current().create_grant(
            env::predecessor_account_id().as_str(),
            &grant_id,
            title,
            holder.as_str(),
            total.0,
            start,
            cliff,
            duration,
            cancellable,
        ));
    }
    pub fn quote(&self, grant_id: String, amount: Option<U128>) -> QuoteView {
        Self::quote_view(&self.current(), &grant_id, amount.map(|a| a.0))
    }
    #[payable]
    pub fn borrow(&mut self, grant_id: String, amount: U128, lender: AccountId) {
        near_sdk::assert_one_yocto();
        self.engine = checked(self.current().borrow(
            env::predecessor_account_id().as_str(),
            &grant_id,
            amount.0,
            lender.as_str(),
        ));
    }
    /// Outbound adapter remains disabled under the public synthetic initializer.
    /// Tests configure it only inside their local fixture; no funded mode is exposed.
    pub fn claim(&mut self, grant_id: String) -> Promise {
        assert!(!self.funded, "the funded contract uses unlock, then withdraw");
        let token = self
            .token_contract
            .clone()
            .unwrap_or_else(|| env::panic_str("FT adapter disabled: real funding not implemented"));
        assert!(
            env::prepaid_gas().as_gas() >= Gas::from_tgas(140).as_gas(),
            "140 Tgas required for transfers, callback and overhead"
        );
        let (model, transfer_id) = checked(
            self.current()
                .claim_ft(env::predecessor_account_id().as_str(), &grant_id),
        );
        let transfer = model.transfers.get(&transfer_id).unwrap();
        let mut promises = transfer.legs.iter().map(|leg| {
            ext_ft::ext(token.clone())
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .with_static_gas(Gas::from_tgas(20))
                .ft_transfer(
                    leg.recipient.parse().expect("validated account"),
                    U128(leg.amount),
                    Some(format!("vesting claim {transfer_id}")),
                )
        });
        let first = promises.next().expect("nonempty claim");
        let combined = promises.fold(first, |joined, next| joined.and(next));
        self.engine = model;
        combined.then(
            Self::ext(env::current_account_id())
                .with_static_gas(Gas::from_tgas(60))
                .on_claim_resolved(transfer_id),
        )
    }
    /// Valid callback failures are visible and never trigger an automatic resend.
    #[private]
    pub fn on_claim_resolved(&mut self, transfer_id: u64) -> bool {
        if env::predecessor_account_id() != env::current_account_id() {
            env::log_str("Unauthorized callback ignored");
            return false;
        }
        let Some(t) = self.engine.transfers.get(&transfer_id) else {
            env::log_str("Unknown callback ignored");
            return false;
        };
        if t.legs.is_empty()
            || !t
                .legs
                .iter()
                .all(|l| l.state == engine::TransferState::Pending)
        {
            env::log_str("Repeated or non-recipient callback ignored");
            return false;
        }
        if env::promise_results_count() != t.legs.len() as u64 {
            self.mark_reconcile(transfer_id, "Unexpected callback result count", vec![]);
            return false;
        }
        let outcomes: Vec<bool> = (0..t.legs.len())
            .map(|i| matches!(env::promise_result(i as u64), PromiseResult::Successful(_)))
            .collect();
        if day() < self.engine.day {
            self.mark_reconcile(transfer_id, "Callback day moved backwards", outcomes);
            return false;
        }
        let mut next = self.engine.clone();
        if let Err(error) = next.advance_to(day()) {
            self.mark_reconcile(transfer_id, &error, outcomes);
            return false;
        }
        for (index, success) in outcomes.iter().enumerate() {
            match next.resolve_ft_leg(
                env::current_account_id().as_str(),
                transfer_id,
                index,
                *success,
            ) {
                Ok(updated) => next = updated,
                Err(error) => {
                    self.mark_reconcile(transfer_id, &error, outcomes.clone());
                    return false;
                }
            }
        }
        self.engine = next;
        outcomes.iter().all(|v| *v)
    }
    pub fn needs_reconcile(&self) -> Vec<ReconcileRecord> {
        self.reconciliations.values().cloned().collect()
    }
    /// Choice 1: sell the next `slice` unlocked tokens of a grant to `buyer` for `price` now.
    #[payable]
    pub fn sell_slice(&mut self, grant_id: String, slice: U128, price: U128, buyer: AccountId) {
        near_sdk::assert_one_yocto();
        self.engine = checked(self.current().sell_slice(
            env::predecessor_account_id().as_str(),
            &grant_id,
            slice.0,
            price.0,
            buyer.as_str(),
        ));
    }
    /// Choice 3: stake the grant's escrow and take the rewards as they arrive.
    pub fn stake(&mut self, grant_id: String) {
        assert!(!self.funded, "staking pool not wired in the funded contract; the demo shows stake");
        self.engine = checked(
            self.current()
                .stake(env::predecessor_account_id().as_str(), &grant_id),
        );
    }
    pub fn collect_rewards(&mut self, grant_id: String) {
        assert!(!self.funded, "rewards would be unbacked in the funded contract");
        self.engine = checked(
            self.current()
                .collect_rewards(env::predecessor_account_id().as_str(), &grant_id),
        );
    }
    #[payable]
    pub fn unstake(&mut self, grant_id: String) {
        near_sdk::assert_one_yocto();
        self.engine = checked(
            self.current()
                .unstake(env::predecessor_account_id().as_str(), &grant_id),
        );
    }
    #[payable]
    pub fn repay(&mut self, grant_id: String, amount: U128) {
        near_sdk::assert_one_yocto();
        self.engine = checked(self.current().repay(
            env::predecessor_account_id().as_str(),
            &grant_id,
            amount.0,
        ));
    }
    #[payable]
    pub fn cancel(&mut self, grant_id: String) {
        near_sdk::assert_one_yocto();
        self.engine = checked(
            self.current()
                .cancel(env::predecessor_account_id().as_str(), &grant_id),
        );
    }
    pub fn get_grants(&self, account_id: AccountId) -> Vec<GrantView> {
        let model = self.current();
        model
            .grants
            .iter()
            .filter(|(_, g)| g.holder == account_id.as_str())
            .map(|(id, g)| Self::grant_view(&model, id, g))
            .collect()
    }
    pub fn options(&self, account_id: AccountId) -> Vec<OptionsView> {
        let model = self.current();
        model
            .grants
            .iter()
            .filter(|(_, g)| g.holder == account_id.as_str())
            .map(|(id, g)| OptionsView {
                grant: Self::grant_view(&model, id, g),
                can_claim: checked(g.claimable(model.day)) > 0 && g.pending == 0,
                borrow: Self::quote_view(&model, id, None),
                can_repay: !model.needs_reconcile(id)
                    && g.lien.as_ref().is_some_and(|l| l.debt > 0),
                can_sell: !model.needs_reconcile(id)
                    && !g.committed()
                    && !g.cancellable
                    && g.cancelled_at.is_none()
                    && checked(g.unvested(model.day)) > 0,
                max_slice: U128(
                    if !g.committed() && !g.cancellable && g.cancelled_at.is_none() {
                        checked(g.unvested(model.day))
                    } else {
                        0
                    },
                ),
                can_stake: !model.needs_reconcile(id)
                    && !g.committed()
                    && g.cancelled_at.is_none()
                    && g.escrow > 0,
                can_collect: g.stake.as_ref().is_some_and(|s| s.unpaid > 0),
            })
            .collect()
    }
    pub fn demo_balance(&self, account_id: AccountId) -> U128 {
        U128(checked(self.engine.balance(account_id.as_str())))
    }
    pub fn accounting_mode(&self) -> String {
        if self.funded {
            return "FUNDED: balances and escrow backed one-for-one by the configured token; see backing()".into();
        }
        "DEMO ONLY: synthetic balances; FT adapter disabled by public initializer; no funded deployment".into()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ft_tests;

#[cfg(test)]
mod choice_tests;

#[cfg(test)]
mod funded_tests;
