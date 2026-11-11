//! Slice-1 accounting model. Units are synthetic; this module does not transfer assets.
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use std::collections::BTreeMap;

pub type Amount = u128;
pub type Result<T> = std::result::Result<T, String>;
const APR: u128 = 12;
/// Demo staking yield in basis points a year (about 4.5%, as reported after NEAR's
/// Oct 2025 inflation cut). A planner input, not a promise; rewards are minted here
/// because no staking pool is wired in.
pub const STAKE_BP: u128 = 450;
fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn add(a: Amount, b: Amount) -> Result<Amount> {
    a.checked_add(b).ok_or("amount overflow".into())
}
fn mul(a: Amount, b: Amount) -> Result<Amount> {
    a.checked_mul(b).ok_or("amount overflow".into())
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Lien {
    pub id: u64,
    pub lender: String,
    pub principal: Amount,
    pub fee: Amount,
    pub debt: Amount,
    pub repaid_by_unlocks: Amount,
    pub repaid_by_holder: Amount,
    pub opened_day: u64,
    pub shortfall: Amount,
    pub closed_day: Option<u64>,
}
/// Choice 1: a buyer paid now for the next `slice` unlocked tokens.
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Sale {
    pub id: u64,
    pub buyer: String,
    pub slice: Amount,
    pub price: Amount,
    pub delivered: Amount,
    pub opened_day: u64,
    pub shortfall: Amount,
    pub closed_day: Option<u64>,
}
/// Choice 3: the escrow earns while the holder keeps every unlock.
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Stake {
    pub id: u64,
    pub opened_day: u64,
    pub yield_bp: u128,
    pub unpaid: Amount,
    pub paid: Amount,
    pub last_day: u64,
    pub closed_day: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Grant {
    pub title: String,
    pub issuer: String,
    pub holder: String,
    pub total: Amount,
    pub start: u64,
    pub cliff: u64,
    pub duration: u64,
    pub cancellable: bool,
    pub cancelled_at: Option<u64>,
    pub escrow: Amount,
    pub claimed: Amount,
    pub pending: Amount,
    pub returned: Amount,
    pub lien: Option<Lien>,
    pub history: Vec<Lien>,
    pub sale: Option<Sale>,
    pub sale_history: Vec<Sale>,
    pub stake: Option<Stake>,
    pub stake_history: Vec<Stake>,
}
impl Grant {
    /// One commitment per grant: a loan, a sold slice or a stake. Never two.
    pub fn committed(&self) -> bool {
        self.lien.is_some() || self.sale.is_some() || self.stake.is_some()
    }
    /// Counterparty, commitment id and tokens still owed from future unlocks.
    pub fn counterparty(&self) -> Option<(String, u64, Amount)> {
        if let Some(l) = &self.lien {
            Some((l.lender.clone(), l.id, l.debt))
        } else {
            self.sale
                .as_ref()
                .map(|s| (s.buyer.clone(), s.id, s.slice - s.delivered))
        }
    }
    fn close_delivered_sale(&mut self, day: u64) {
        if self.sale.as_ref().is_some_and(|s| s.delivered == s.slice) {
            let mut s = self.sale.take().unwrap();
            s.closed_day = Some(day);
            s.shortfall = 0;
            self.sale_history.push(s);
        }
    }
    pub fn vested(&self, day: u64) -> Result<Amount> {
        let elapsed = day
            .min(self.cancelled_at.unwrap_or(day))
            .saturating_sub(self.start);
        if elapsed < self.cliff {
            return Ok(0);
        }
        Ok(mul(self.total, elapsed.min(self.duration).into())? / u128::from(self.duration))
    }
    pub fn claimable(&self, day: u64) -> Result<Amount> {
        self.vested(day)?
            .checked_sub(add(self.claimed, self.pending)?)
            .ok_or("over-claimed".into())
    }
    pub fn unvested(&self, day: u64) -> Result<Amount> {
        if self.cancelled_at.is_some() {
            Ok(0)
        } else {
            Ok(self.total - self.vested(day)?)
        }
    }
    fn close_paid_lien(&mut self, day: u64) {
        if self.lien.as_ref().is_some_and(|l| l.debt == 0) {
            let mut l = self.lien.take().unwrap();
            l.closed_day = Some(day);
            l.shortfall = 0;
            self.history.push(l);
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quote {
    pub ltv_pct: u8,
    pub max_advance: Amount,
    pub term_days: u64,
}
impl Quote {
    pub fn fee_for(&self, principal: Amount) -> Result<Amount> {
        Ok(mul(mul(principal, APR)?, self.term_days.into())? / (100 * 365))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub enum TransferState {
    Pending,
    Done,
    FailedRestored,
    Mixed,
    NeedsReconcile,
}
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct TransferLeg {
    pub recipient: String,
    pub amount: Amount,
    pub lender_leg: bool,
    pub state: TransferState,
}
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Transfer {
    pub grant: String,
    pub amount: Amount,
    pub lender: Option<String>,
    pub reserved_for_lender: Amount,
    pub legs: Vec<TransferLeg>,
    pub lien_id: Option<u64>,
    pub state: TransferState,
}
#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct Engine {
    pub day: u64,
    pub contract: String,
    pub wallets: BTreeMap<String, Amount>,
    pub grants: BTreeMap<String, Grant>,
    pub transfers: BTreeMap<u64, Transfer>,
    pub supply: Amount,
    /// What each account has agreed to lend or pay for a slice. A holder can draw on nothing else.
    pub lendable: BTreeMap<String, Amount>,
    /// Withdrawals whose token transfer has not resolved yet. Their wallets are never evicted.
    pub withdrawing: BTreeMap<String, Amount>,
    next_transfer: u64,
    next_lien: u64,
}
impl Engine {
    pub fn new(contract: String, wallets: BTreeMap<String, Amount>, day: u64) -> Result<Self> {
        need(wallets.len() <= 64, "demo wallet limit")?;
        let supply = wallets.values().try_fold(0, |a, &b| add(a, b))?;
        let lendable = wallets.clone();
        Ok(Self {
            lendable,
            withdrawing: BTreeMap::new(),
            day,
            contract,
            wallets,
            grants: BTreeMap::new(),
            transfers: BTreeMap::new(),
            supply,
            next_transfer: 1,
            next_lien: 1,
        })
    }
    fn draw_offer(&mut self, account: &str, amount: Amount, message: &str) -> Result<()> {
        let offer = self.lendable.get(account).copied().unwrap_or(0);
        need(amount <= offer, message)?;
        if offer == amount {
            self.lendable.remove(account);
        } else {
            self.lendable.insert(account.into(), offer - amount);
        }
        Ok(())
    }
    /// Set how much of `account`'s balance a holder may draw as a loan or a slice payment. Only the account itself calls this.
    pub fn set_lendable(&self, account: &str, amount: Amount) -> Result<Self> {
        need(self.wallets.contains_key(account), "unknown account")?;
        let mut s = self.clone();
        if amount == 0 {
            s.lendable.remove(account);
        } else {
            s.lendable.insert(account.into(), amount);
        }
        s.checked()
    }
    pub fn lendable_of(&self, account: &str) -> Amount {
        self.lendable.get(account).copied().unwrap_or(0)
    }
    /// A settled transfer has nothing in flight; dropping it keeps unlocks working past the retained limit.
    fn prune_settled_transfers(&mut self) {
        let live = |st: &TransferState| matches!(st, TransferState::Pending | TransferState::NeedsReconcile);
        self.transfers.retain(|_, t| {
            if t.legs.is_empty() { live(&t.state) } else { live(&t.state) || t.legs.iter().any(|l| live(&l.state)) }
        });
    }
    /// Free one wallet slot held by an empty account that no grant, loan, sale or offer refers to.
    fn evict_idle_wallet(&mut self) {
        let withdrawing = &self.withdrawing;
        let referenced = |a: &str, grants: &BTreeMap<String, Grant>, lendable: &BTreeMap<String, Amount>| {
            lendable.contains_key(a)
                || withdrawing.contains_key(a)
                || grants.values().any(|g| {
                    g.holder == a || g.issuer == a || g.counterparty().is_some_and(|(who, _, _)| who == a)
                })
        };
        let idle = self
            .wallets
            .iter()
            .find(|(a, &b)| b == 0 && *a != &self.contract && !referenced(a, &self.grants, &self.lendable))
            .map(|(a, _)| a.clone());
        if let Some(a) = idle {
            self.wallets.remove(&a);
        }
    }
    pub fn needs_reconcile(&self, id: &str) -> bool {
        self.transfers
            .values()
            .any(|t| t.grant == id && t.state == TransferState::NeedsReconcile)
    }
    pub fn balance(&self, account: &str) -> Result<Amount> {
        self.wallets
            .get(account)
            .copied()
            .ok_or("unknown demo account".into())
    }
    fn debit(&mut self, account: &str, amount: Amount) -> Result<()> {
        let value = self
            .balance(account)?
            .checked_sub(amount)
            .ok_or("insufficient balance")?;
        self.wallets.insert(account.into(), value);
        Ok(())
    }
    fn credit(&mut self, account: &str, amount: Amount) -> Result<()> {
        let value = add(self.balance(account)?, amount)?;
        self.wallets.insert(account.into(), value);
        Ok(())
    }
    pub fn grant(&self, id: &str) -> Result<&Grant> {
        self.grants.get(id).ok_or("unknown grant".into())
    }
    pub fn quote(&self, id: &str) -> Result<Quote> {
        let g = self.grant(id)?;
        let ltv = if g.cancellable { 0 } else { 40 };
        let end = g.start.checked_add(g.duration).ok_or("schedule overflow")?;
        let max_advance =
            if g.lien.is_some() || g.cancelled_at.is_some() || self.needs_reconcile(id) {
                0
            } else if g.sale.is_some() || g.stake.is_some() {
                0
            } else {
                mul(g.unvested(self.day)?, ltv)? / 100
            };
        Ok(Quote {
            ltv_pct: ltv as u8,
            max_advance,
            term_days: end.saturating_sub(self.day),
        })
    }
    /// Staked escrow earns for the days since it was last counted. Call before escrow changes.
    pub fn accrue_stakes(&mut self) -> Result<()> {
        let day = self.day;
        for g in self.grants.values_mut() {
            let escrow = g.escrow;
            if let Some(st) = &mut g.stake {
                let days = u128::from(day.saturating_sub(st.last_day));
                st.unpaid = add(st.unpaid, mul(mul(escrow, st.yield_bp)?, days)? / (10_000 * 365))?;
                st.last_day = st.last_day.max(day);
            }
        }
        Ok(())
    }
    /// Move the clock forward, counting stake rewards on the escrow held until now.
    pub fn advance_to(&mut self, day: u64) -> Result<()> {
        need(day >= self.day, "day moved backwards")?;
        let mut s = self.clone();
        s.day = day;
        s.accrue_stakes()?;
        *self = s.checked()?;
        Ok(())
    }
    fn checked(mut self) -> Result<Self> {
        let updates: Vec<_> = self
            .grants
            .iter()
            .filter_map(|(id, g)| {
                g.counterparty()
                    .map(|(_, cid, owed)| (id.clone(), cid, owed, g.cancelled_at.is_some()))
            })
            .collect();
        for (id, cid, owed, cancelled) in updates {
            let reserved = self.reserved_debt(&id, cid)?;
            let claimable = self.grant(&id)?.claimable(self.day)?;
            let shortfall = if cancelled {
                owed.saturating_sub(add(reserved, claimable)?)
            } else {
                0
            };
            let g = self.grants.get_mut(&id).unwrap();
            if let Some(l) = &mut g.lien {
                l.shortfall = shortfall;
            } else if let Some(s) = &mut g.sale {
                s.shortfall = shortfall;
            }
        }
        self.assert_invariants()?;
        Ok(self)
    }
    pub fn assert_invariants(&self) -> Result<()> {
        need(
            self.wallets.len() <= 64 && self.grants.len() <= 32 && self.transfers.len() <= 128,
            "bounded state limit",
        )?;
        let wallets = self.wallets.values().try_fold(0, |a, &b| add(a, b))?;
        let mut escrow = 0;
        let mut pending = 0;
        for g in self.grants.values() {
            need(
                g.history.len() <= 32 && g.sale_history.len() <= 32 && g.stake_history.len() <= 32,
                "lien history limit",
            )?;
            need(
                u8::from(g.lien.is_some()) + u8::from(g.sale.is_some()) + u8::from(g.stake.is_some()) <= 1,
                "one commitment per grant",
            )?;
            escrow = add(escrow, g.escrow)?;
            pending = add(pending, g.pending)?;
            need(
                add(add(g.escrow, g.pending)?, add(g.claimed, g.returned)?)? == g.total,
                "grant conservation",
            )?;
            g.claimable(self.day)?;
        }
        let mut in_flight = 0;
        for t in self.transfers.values() {
            if t.legs.is_empty() {
                if matches!(
                    t.state,
                    TransferState::Pending | TransferState::NeedsReconcile
                ) {
                    in_flight = add(in_flight, t.amount)?;
                }
            } else {
                need(
                    t.legs.iter().try_fold(0, |a, l| add(a, l.amount))? == t.amount,
                    "leg conservation",
                )?;
                for leg in &t.legs {
                    if matches!(
                        leg.state,
                        TransferState::Pending | TransferState::NeedsReconcile
                    ) {
                        in_flight = add(in_flight, leg.amount)?;
                    }
                }
            }
        }
        for (id, g) in &self.grants {
            if let Some((_, cid, owed)) = g.counterparty() {
                need(
                    self.reserved_debt(id, cid)? <= owed,
                    "reserved debt exceeds debt",
                )?;
            }
        }
        need(pending == in_flight, "pending conservation")?;
        need(
            add(add(wallets, escrow)?, pending)? == self.supply,
            "total conservation",
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn create_grant(
        &self,
        actor: &str,
        id: &str,
        title: String,
        holder: &str,
        total: Amount,
        start: u64,
        cliff: u64,
        duration: u64,
        cancellable: bool,
    ) -> Result<Self> {
        need(!self.grants.contains_key(id), "duplicate grant")?;
        need(self.grants.len() < 32, "grant limit")?;
        need(
            !id.is_empty() && id.len() <= 100 && title.len() <= 256,
            "invalid grant label",
        )?;
        need(
            total > 0 && duration > 0 && cliff <= duration,
            "invalid schedule or total",
        )?;
        start.checked_add(duration).ok_or("schedule overflow")?;
        self.balance(holder)?;
        let mut s = self.clone();
        s.debit(actor, total)?;
        s.grants.insert(
            id.into(),
            Grant {
                title,
                issuer: actor.into(),
                holder: holder.into(),
                total,
                start,
                cliff,
                duration,
                cancellable,
                cancelled_at: None,
                escrow: total,
                claimed: 0,
                pending: 0,
                returned: 0,
                lien: None,
                history: vec![],
                sale: None,
                sale_history: vec![],
                stake: None,
                stake_history: vec![],
            },
        );
        s.checked()
    }
    pub fn borrow(&self, actor: &str, id: &str, amount: Amount, lender: &str) -> Result<Self> {
        need(!self.needs_reconcile(id), "grant needs reconciliation")?;
        let mut g = self.grant(id)?.clone();
        need(actor == g.holder, "only holder borrows")?;
        need(!g.committed(), "already committed")?;
        need(g.history.len() < 32, "lien history limit")?;
        need(amount > 0, "positive amount required")?;
        let q = self.quote(id)?;
        need(amount <= q.max_advance, "above borrow limit")?;
        need(lender != actor, "holder cannot lend to self")?;
        let fee = q.fee_for(amount)?;
        let debt = add(amount, fee)?;
        let mut s = self.clone();
        s.draw_offer(lender, amount, "lender has not offered this much")?;
        s.debit(lender, amount)?;
        s.credit(actor, amount)?;
        let lien_id = s.next_lien;
        s.next_lien = lien_id.checked_add(1).ok_or("lien id overflow")?;
        g.lien = Some(Lien {
            id: lien_id,
            lender: lender.into(),
            principal: amount,
            fee,
            debt,
            repaid_by_unlocks: 0,
            repaid_by_holder: 0,
            opened_day: self.day,
            shortfall: 0,
            closed_day: None,
        });
        s.grants.insert(id.into(), g);
        s.checked()
    }
    fn reserve_claim(&self, actor: &str, id: &str) -> Result<(Self, u64)> {
        need(!self.needs_reconcile(id), "grant needs reconciliation")?;
        let mut g = self.grant(id)?.clone();
        need(
            actor == g.holder || g.counterparty().is_some_and(|(who, _, _)| who == actor),
            "only holder, lender or buyer claims",
        )?;
        need(g.pending == 0, "claim already pending")?;
        let mut s = self.clone();
        s.prune_settled_transfers();
        need(s.transfers.len() < 128, "retained transfer limit")?;
        let amount = g.claimable(self.day)?;
        need(amount > 0, "nothing claimable")?;
        let (lender, reserved, lien_id) = match g.counterparty() {
            Some((who, cid, owed)) => (Some(who), amount.min(owed), Some(cid)),
            None => (None, 0, None),
        };
        g.escrow = g.escrow.checked_sub(amount).ok_or("insufficient escrow")?;
        g.pending = add(g.pending, amount)?;
        let tid = s.next_transfer;
        s.next_transfer = tid.checked_add(1).ok_or("transfer id overflow")?;
        s.transfers.insert(
            tid,
            Transfer {
                grant: id.into(),
                amount,
                lender,
                reserved_for_lender: reserved,
                legs: vec![],
                lien_id,
                state: TransferState::Pending,
            },
        );
        s.grants.insert(id.into(), g);
        Ok((s.checked()?, tid))
    }
    // Slice-1 atomic simulator retained solely as the archived comparison in tests.
    #[cfg(test)]
    pub fn claim(&self, actor: &str, id: &str) -> Result<(Self, u64)> {
        self.reserve_claim(actor, id)
    }
    #[cfg(test)]
    pub fn resolve(&self, actor: &str, tid: u64, success: bool) -> Result<Self> {
        need(actor == self.contract, "callback must be contract itself")?;
        let mut t = self.transfers.get(&tid).ok_or("unknown transfer")?.clone();
        need(
            t.state == TransferState::Pending && t.legs.is_empty(),
            "atomic simulation only",
        )?;
        let mut g = self.grant(&t.grant)?.clone();
        g.pending = g.pending.checked_sub(t.amount).ok_or("pending underflow")?;
        let mut s = self.clone();
        if success {
            let mut pay = 0;
            if let Some(l) = &mut g.lien {
                // A repayment can close a lien while the claim is pending. Never pay a later loan.
                if Some(l.id) == t.lien_id && Some(&l.lender) == t.lender.as_ref() {
                    pay = t.reserved_for_lender.min(l.debt);
                    l.debt -= pay;
                    l.repaid_by_unlocks = add(l.repaid_by_unlocks, pay)?;
                }
            }
            if pay > 0 {
                s.credit(t.lender.as_deref().unwrap(), pay)?;
            }
            s.credit(&g.holder, t.amount - pay)?;
            g.claimed = add(g.claimed, t.amount)?;
            t.state = TransferState::Done;
        } else {
            g.escrow = add(g.escrow, t.amount)?;
            t.state = TransferState::FailedRestored;
        }
        g.close_paid_lien(self.day);
        s.grants.insert(t.grant.clone(), g);
        s.transfers.insert(tid, t);
        s.checked()
    }
    pub fn reserved_debt(&self, grant_id: &str, lien_id: u64) -> Result<Amount> {
        self.transfers
            .values()
            .filter(|t| t.grant == grant_id && t.lien_id == Some(lien_id))
            .flat_map(|t| &t.legs)
            .filter(|l| {
                l.lender_leg
                    && matches!(
                        l.state,
                        TransferState::Pending | TransferState::NeedsReconcile
                    )
            })
            .try_fold(0, |a, l| add(a, l.amount))
    }
    pub fn claim_ft(&self, actor: &str, id: &str) -> Result<(Self, u64)> {
        let (mut s, tid) = self.reserve_claim(actor, id)?;
        let holder = s.grant(id)?.holder.clone();
        let t = s.transfers.get_mut(&tid).unwrap();
        if t.reserved_for_lender > 0 {
            t.legs.push(TransferLeg {
                recipient: t.lender.clone().unwrap(),
                amount: t.reserved_for_lender,
                lender_leg: true,
                state: TransferState::Pending,
            });
        }
        if t.amount > t.reserved_for_lender {
            t.legs.push(TransferLeg {
                recipient: holder,
                amount: t.amount - t.reserved_for_lender,
                lender_leg: false,
                state: TransferState::Pending,
            });
        }
        Ok((s.checked()?, tid))
    }
    pub fn resolve_ft_leg(
        &self,
        actor: &str,
        tid: u64,
        index: usize,
        success: bool,
    ) -> Result<Self> {
        need(actor == self.contract, "callback must be contract itself")?;
        let mut t = self.transfers.get(&tid).ok_or("unknown transfer")?.clone();
        let leg = t.legs.get_mut(index).ok_or("unknown transfer leg")?;
        need(
            leg.state == TransferState::Pending,
            "transfer leg already resolved",
        )?;
        let mut g = self.grant(&t.grant)?.clone();
        g.pending = g
            .pending
            .checked_sub(leg.amount)
            .ok_or("pending underflow")?;
        let mut s = self.clone();
        if success {
            if leg.lender_leg {
                if let Some(l) = g.lien.as_mut() {
                    need(
                        Some(l.id) == t.lien_id && Some(&l.lender) == t.lender.as_ref(),
                        "reserved lien generation changed",
                    )?;
                    l.debt = l
                        .debt
                        .checked_sub(leg.amount)
                        .ok_or("reserved debt underflow")?;
                    l.repaid_by_unlocks = add(l.repaid_by_unlocks, leg.amount)?;
                } else {
                    let sale = g.sale.as_mut().ok_or("reserved lien missing")?;
                    need(
                        Some(sale.id) == t.lien_id && Some(&sale.buyer) == t.lender.as_ref(),
                        "reserved lien generation changed",
                    )?;
                    need(
                        sale.slice - sale.delivered >= leg.amount,
                        "reserved debt underflow",
                    )?;
                    sale.delivered = add(sale.delivered, leg.amount)?;
                }
            }
            s.credit(&leg.recipient, leg.amount)?;
            g.claimed = add(g.claimed, leg.amount)?;
            leg.state = TransferState::Done;
        } else {
            g.escrow = add(g.escrow, leg.amount)?;
            leg.state = TransferState::FailedRestored;
        }
        t.state = if t.legs.iter().any(|l| l.state == TransferState::Pending) {
            TransferState::Pending
        } else if t.legs.iter().all(|l| l.state == TransferState::Done) {
            TransferState::Done
        } else if t
            .legs
            .iter()
            .all(|l| l.state == TransferState::FailedRestored)
        {
            TransferState::FailedRestored
        } else {
            TransferState::Mixed
        };
        g.close_paid_lien(self.day);
        g.close_delivered_sale(self.day);
        s.grants.insert(t.grant.clone(), g);
        s.transfers.insert(tid, t);
        s.checked()
    }
    pub fn repay(&self, actor: &str, id: &str, amount: Amount) -> Result<Self> {
        need(!self.needs_reconcile(id), "grant needs reconciliation")?;
        let mut g = self.grant(id)?.clone();
        need(actor == g.holder, "only holder repays")?;
        let lien = g.lien.as_ref().ok_or("no debt")?;
        let available = lien
            .debt
            .checked_sub(self.reserved_debt(id, lien.id)?)
            .ok_or("reserved debt overflow")?;
        need(
            amount > 0 && amount <= available,
            "repayment exceeds unreserved debt",
        )?;
        let l = g.lien.as_mut().unwrap();
        let mut s = self.clone();
        s.debit(actor, amount)?;
        s.credit(&l.lender, amount)?;
        l.debt -= amount;
        l.repaid_by_holder = add(l.repaid_by_holder, amount)?;
        g.close_paid_lien(self.day);
        s.grants.insert(id.into(), g);
        s.checked()
    }
    pub fn cancel(&self, actor: &str, id: &str) -> Result<Self> {
        need(!self.needs_reconcile(id), "grant needs reconciliation")?;
        let mut g = self.grant(id)?.clone();
        need(actor == g.issuer, "only issuer cancels")?;
        need(
            g.cancellable && g.cancelled_at.is_none(),
            "grant not cancellable or already cancelled",
        )?;
        g.cancelled_at = Some(self.day);
        let mut rewards = 0;
        if let Some(mut st) = g.stake.take() {
            // Cancellation ends the stake; rewards already earned still go to the holder.
            rewards = st.unpaid;
            st.paid = add(st.paid, rewards)?;
            st.unpaid = 0;
            st.closed_day = Some(self.day);
            g.stake_history.push(st);
        }
        let claimable = g.claimable(self.day)?;
        let give_back = g
            .escrow
            .checked_sub(claimable)
            .ok_or("invalid cancel escrow")?;
        g.escrow -= give_back;
        g.returned = add(g.returned, give_back)?;
        if let Some(l) = &mut g.lien {
            l.shortfall = l.debt.saturating_sub(claimable);
        }
        if let Some(sale) = &mut g.sale {
            sale.shortfall = (sale.slice - sale.delivered).saturating_sub(claimable);
        }
        let holder = g.holder.clone();
        let mut s = self.clone();
        s.credit(actor, give_back)?;
        if rewards > 0 {
            s.credit(&holder, rewards)?;
            s.supply = add(s.supply, rewards)?;
        }
        s.grants.insert(id.into(), g);
        s.checked()
    }
    fn next_commitment_id(&mut self) -> Result<u64> {
        let id = self.next_lien;
        self.next_lien = id.checked_add(1).ok_or("lien id overflow")?;
        Ok(id)
    }
    /// Choice 1: sell the next `slice` unlocked tokens to `buyer` for `price` now.
    pub fn sell_slice(
        &self,
        actor: &str,
        id: &str,
        slice: Amount,
        price: Amount,
        buyer: &str,
    ) -> Result<Self> {
        need(!self.needs_reconcile(id), "grant needs reconciliation")?;
        let mut g = self.grant(id)?.clone();
        need(actor == g.holder, "only holder sells")?;
        need(!g.committed(), "already committed")?;
        need(g.sale_history.len() < 32, "lien history limit")?;
        need(
            !g.cancellable && g.cancelled_at.is_none(),
            "only a grant the issuer cannot cancel can be sold",
        )?;
        need(
            slice > 0 && slice <= g.unvested(self.day)?,
            "slice must be within the unvested amount",
        )?;
        need(price > 0 && price <= slice, "price must be positive and at most the slice")?;
        need(buyer != actor, "holder cannot buy from self")?;
        let mut s = self.clone();
        s.draw_offer(buyer, price, "buyer has not offered this much")?;
        s.debit(buyer, price)?;
        s.credit(actor, price)?;
        let sale_id = s.next_commitment_id()?;
        g.sale = Some(Sale {
            id: sale_id,
            buyer: buyer.into(),
            slice,
            price,
            delivered: 0,
            opened_day: self.day,
            shortfall: 0,
            closed_day: None,
        });
        s.grants.insert(id.into(), g);
        s.checked()
    }
    /// Choice 3: stake the escrow; the holder keeps every unlock and takes the rewards.
    pub fn stake(&self, actor: &str, id: &str) -> Result<Self> {
        need(!self.needs_reconcile(id), "grant needs reconciliation")?;
        let mut g = self.grant(id)?.clone();
        need(actor == g.holder, "only holder stakes")?;
        need(!g.committed(), "already committed")?;
        need(g.stake_history.len() < 32, "lien history limit")?;
        need(g.cancelled_at.is_none() && g.escrow > 0, "nothing left to stake")?;
        let mut s = self.clone();
        let stake_id = s.next_commitment_id()?;
        g.stake = Some(Stake {
            id: stake_id,
            opened_day: self.day,
            yield_bp: STAKE_BP,
            unpaid: 0,
            paid: 0,
            last_day: self.day,
            closed_day: None,
        });
        s.grants.insert(id.into(), g);
        s.checked()
    }
    pub fn collect_rewards(&self, actor: &str, id: &str) -> Result<Self> {
        let mut g = self.grant(id)?.clone();
        need(actor == g.holder, "only holder collects")?;
        let st = g.stake.as_mut().ok_or("not staked")?;
        need(st.unpaid > 0, "no rewards to collect")?;
        let reward = st.unpaid;
        st.unpaid = 0;
        st.paid = add(st.paid, reward)?;
        let mut s = self.clone();
        s.credit(actor, reward)?;
        s.supply = add(s.supply, reward)?;
        s.grants.insert(id.into(), g);
        s.checked()
    }
    // ---------- Funded mode: wallets are custodial balances backed one-for-one by real tokens ----------
    /// Real tokens arrived from `account` (NEP-141 ft_on_transfer). Supply grows by exactly that amount.
    pub fn deposit(&self, account: &str, amount: Amount) -> Result<Self> {
        need(amount > 0, "empty deposit")?;
        let s = self.ensure_wallet(account)?;
        let mut s = s;
        s.credit(account, amount)?;
        s.supply = add(s.supply, amount)?;
        s.checked()
    }
    /// Register an account with a zero balance so it can hold a grant or receive an unlock.
    pub fn ensure_wallet(&self, account: &str) -> Result<Self> {
        need(!account.is_empty() && account.len() <= 64, "invalid account")?;
        let mut s = self.clone();
        if !s.wallets.contains_key(account) {
            if s.wallets.len() >= 64 {
                s.evict_idle_wallet();
            }
            need(s.wallets.len() < 64, "demo wallet limit")?;
            s.wallets.insert(account.into(), 0);
        }
        Ok(s)
    }
    /// Unlock: settle a claim inside the contract. The lender or buyer is paid first, the holder gets the rest,
    /// all as custodial balances. Tokens leave the contract only through `withdraw_reserve`.
    pub fn unlock_internal(&self, actor: &str, id: &str) -> Result<Self> {
        let (mut s, tid) = self.claim_ft(actor, id)?;
        let legs = s.transfers.get(&tid).map(|t| t.legs.len()).unwrap_or(0);
        let contract = s.contract.clone();
        for i in 0..legs {
            s = s.resolve_ft_leg(&contract, tid, i, true)?;
        }
        Ok(s)
    }
    /// Take `amount` out of a custodial balance before the outgoing token transfer. Supply shrinks with it.
    pub fn withdraw_reserve(&self, account: &str, amount: Amount) -> Result<Self> {
        need(amount > 0, "empty withdrawal")?;
        let mut s = self.clone();
        s.debit(account, amount)?;
        s.supply = s.supply.checked_sub(amount).ok_or("supply underflow")?;
        let open = s.withdrawing.get(account).copied().unwrap_or(0);
        s.withdrawing.insert(account.into(), add(open, amount)?);
        s.checked()
    }
    fn close_withdrawal(&mut self, account: &str, amount: Amount) -> Result<()> {
        let open = self.withdrawing.get(account).copied().unwrap_or(0);
        let left = open.checked_sub(amount).ok_or("no such withdrawal in flight")?;
        if left == 0 {
            self.withdrawing.remove(account);
        } else {
            self.withdrawing.insert(account.into(), left);
        }
        Ok(())
    }
    /// The outgoing token transfer succeeded. Only the contract's own callback may do this.
    pub fn withdraw_settled(&self, actor: &str, account: &str, amount: Amount) -> Result<Self> {
        need(actor == self.contract, "callback must be contract itself")?;
        let mut s = self.clone();
        s.close_withdrawal(account, amount)?;
        s.checked()
    }
    /// The outgoing token transfer failed: put the balance back. Only the contract's own callback may do this.
    pub fn withdraw_restore(&self, actor: &str, account: &str, amount: Amount) -> Result<Self> {
        need(actor == self.contract, "callback must be contract itself")?;
        let mut s = self.ensure_wallet(account)?;
        s.close_withdrawal(account, amount)?;
        s.credit(account, amount)?;
        s.supply = add(s.supply, amount)?;
        s.checked()
    }
    pub fn unstake(&self, actor: &str, id: &str) -> Result<Self> {
        let mut g = self.grant(id)?.clone();
        need(actor == g.holder, "only holder unstakes")?;
        let mut st = g.stake.take().ok_or("not staked")?;
        need(st.unpaid == 0, "collect rewards first")?;
        st.closed_day = Some(self.day);
        g.stake_history.push(st);
        let mut s = self.clone();
        s.grants.insert(id.into(), g);
        s.checked()
    }
}
