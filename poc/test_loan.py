import os, sys, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import loan
from loan import apply, Rejected, initial, options


def holder(s, gid='g1'):
    return next(o for o in options(s, 'alice') if o['id'] == gid)


class LoanJourney(unittest.TestCase):
    def test_entry_shows_grants_and_options(self):
        s = initial()
        o = {x['id']: x for x in options(s, 'alice')}
        self.assertEqual(o['g1']['vested'], 0)
        self.assertEqual(o['g1']['unvested'], 365000)
        self.assertEqual(o['g1']['options']['borrow']['max_advance'], 146000)
        self.assertFalse(o['g2']['options']['borrow']['available'])
        self.assertEqual(o['g2']['options']['borrow']['why_not'], 'cancellable grants cannot back a loan')

    def test_full_self_repaying_path(self):
        s = initial()
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 100000})
        self.assertEqual(s['wallets']['alice'], 100000)
        debt = s['grants']['g1']['lien']['debt']
        self.assertEqual(debt, 112000)  # 12% flat over 365 days
        s = apply(s, 'advance', {'actor': 'alice', 'days': 120})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': True})
        self.assertEqual(s['wallets']['lena.lender'], 800000 - 100000 + 112000)
        self.assertIsNone(s['grants']['g1']['lien'])          # 120k vested > 112k debt
        self.assertEqual(s['wallets']['alice'], 100000 + 120000 - 112000)

    def test_no_double_pledge_and_limit(self):
        s = initial()
        with self.assertRaises(Rejected):
            apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 146001})
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 1000})
        with self.assertRaises(Rejected):
            apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 1000})

    def test_wrong_actor_rejected_and_state_unchanged(self):
        s = initial()
        for action, data in [('borrow', {'actor': 'lena.lender', 'grant': 'g1', 'amount': 10}),
                             ('cancel', {'actor': 'alice', 'grant': 'g2'}),
                             ('cancel', {'actor': 'dao.treasury', 'grant': 'g1'})]:
            with self.assertRaises(Rejected):
                apply(s, action, data)
        self.assertEqual(s, initial())

    def test_failed_transfer_restores_once(self):
        s = initial()
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        with self.assertRaises(Rejected):
            apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})   # cannot claim pending twice
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': False})
        self.assertEqual(s['grants']['g1']['escrow'], 365000)
        with self.assertRaises(Rejected):
            apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': False})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't2', 'ok': True})
        self.assertEqual(s['wallets']['alice'], 100000)

    def test_early_repay_releases_lien(self):
        s = initial()
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 50000})
        debt = s['grants']['g1']['lien']['debt']
        s['wallets']['alice'] += 0
        s = apply(s, 'repay', {'actor': 'alice', 'grant': 'g1', 'amount': 50000})
        with self.assertRaises(Rejected):   # not enough wallet for remaining fee
            apply(s, 'repay', {'actor': 'alice', 'grant': 'g1', 'amount': debt - 50000})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 200})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': True})
        self.assertIsNone(s['grants']['g1']['lien'])
        self.assertEqual(len(s['grants']['g1']['history']), 1)

    def test_repay_while_claim_pending_never_overpays_lender(self):
        s = initial()
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 10000})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        debt = s['grants']['g1']['lien']['debt']             # 11,200; the claim reserves all of it
        with self.assertRaises(Rejected):
            apply(s, 'repay', {'actor': 'alice', 'grant': 'g1', 'amount': 10000})
        self.assertFalse(holder(s)['options']['repay'])
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': True})
        self.assertEqual(s['wallets']['lena.lender'], 800000 - 10000 + debt)   # paid exactly once
        self.assertIsNone(s['grants']['g1']['lien'])

    def test_cancel_returns_unvested_and_shows_shortfall(self):
        s = initial()
        s['grants']['g2']['cancellable'] = False      # borrow first, as if terms allowed it
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g2', 'amount': 40000})
        s['grants']['g2']['cancellable'] = True
        s = apply(s, 'advance', {'actor': 'alice', 'days': 60})
        s = apply(s, 'cancel', {'actor': 'dao.treasury', 'grant': 'g2'})
        g = s['grants']['g2']
        self.assertEqual(g['escrow'], 40000)               # 60/180 of 120k vested, unclaimed
        self.assertEqual(g['lien']['shortfall'], g['lien']['debt'] - 40000)
        self.assertGreater(g['lien']['shortfall'], 0)

    def test_full_repay_while_claim_pending_does_not_pay_lender_twice(self):
        s = initial()
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 140000})   # debt larger than one claim
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g2'})            # alice earns cash from g2
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g2', 'transfer': 't1', 'ok': True})
        debt = s['grants']['g1']['lien']['debt']
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        res = holder(s)['lien']['reserved']
        self.assertGreater(res, 0)
        with self.assertRaises(Rejected):                    # the full debt cannot be repaid while reserved
            apply(s, 'repay', {'actor': 'alice', 'grant': 'g1', 'amount': debt})
        lender0 = s['wallets']['lena.lender']
        s = apply(s, 'repay', {'actor': 'alice', 'grant': 'g1', 'amount': debt - res})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't2', 'ok': True})
        self.assertIsNone(s['grants']['g1']['lien'])
        self.assertEqual(s['wallets']['lena.lender'], lender0 + debt)        # exactly the debt, once

    def test_failed_lender_leg_releases_reservation(self):
        s = initial()
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 10000})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        self.assertFalse(holder(s)['options']['repay'])
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': False})
        self.assertEqual(holder(s)['lien']['reserved'], 0)
        self.assertTrue(holder(s)['options']['repay'])

    def test_lien_cannot_close_or_be_replaced_while_claim_pending(self):
        s = initial()
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 10000})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g2'})            # cash so the wallet is not the blocker
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g2', 'transfer': 't1', 'ok': True})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        self.assertGreater(s['wallets']['alice'], s['grants']['g1']['lien']['debt'])
        with self.assertRaises(Rejected):
            apply(s, 'repay', {'actor': 'alice', 'grant': 'g1', 'amount': s['grants']['g1']['lien']['debt']})
        with self.assertRaises(Rejected):                    # still pledged, so no second loan
            apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 5000})

    def test_cancel_with_pending_claim_counts_the_reserve(self):
        s = initial()
        s['grants']['g2']['cancellable'] = False
        s = apply(s, 'borrow', {'actor': 'alice', 'grant': 'g2', 'amount': 40000})
        s['grants']['g2']['cancellable'] = True
        s = apply(s, 'advance', {'actor': 'alice', 'days': 60})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g2'})
        res = holder(s, 'g2')['lien']['reserved']
        s = apply(s, 'cancel', {'actor': 'dao.treasury', 'grant': 'g2'})
        debt = s['grants']['g2']['lien']['debt']
        self.assertEqual(s['grants']['g2']['lien']['shortfall'], max(0, debt - res))
        ok = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g2', 'transfer': 't1', 'ok': True})
        bad = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g2', 'transfer': 't1', 'ok': False})
        for x in (ok, bad):                                  # same shortfall either way: the reserve becomes paid or claimable again
            l = x['grants']['g2']['lien']
            self.assertEqual(l['shortfall'], max(0, l['debt'] - loan.claimable(x['grants']['g2'], x['day'])))
        self.assertEqual(ok['grants']['g2']['lien']['shortfall'], s['grants']['g2']['lien']['shortfall'])
        self.assertEqual(bad['grants']['g2']['lien']['shortfall'], s['grants']['g2']['lien']['shortfall'])

if __name__ == '__main__':
    unittest.main()
