"""Choices 1 and 3 beside the loan: sell a slice of future unlocks, and stake and earn."""
import os, sys, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import loan
from loan import apply, Rejected, initial, options


def card(s, gid='g1'):
    return next(o for o in options(s, 'alice') if o['id'] == gid)


def sold(s, sl=50000, price=30000):
    return apply(s, 'sell', {'actor': 'alice', 'grant': 'g1', 'slice': sl, 'price': price})


class SellSlice(unittest.TestCase):
    def test_buyer_pays_now_and_is_paid_first_until_the_slice_is_delivered(self):
        s = sold(initial())
        self.assertEqual(s['wallets']['alice'], 30000)
        self.assertEqual(s['wallets']['lena.lender'], 800000 - 30000)
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})              # 100k vested
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': True})
        self.assertEqual(s['wallets']['lena.lender'], 800000 - 30000 + 50000)   # exactly the slice
        self.assertEqual(s['wallets']['alice'], 30000 + 50000)                 # the rest of the unlock
        self.assertIsNone(s['grants']['g1']['sale'])
        self.assertEqual(s['grants']['g1']['history'][-1]['delivered'], 50000)

    def test_partial_unlocks_deliver_the_slice_over_time(self):
        s = sold(initial(), sl=120000, price=70000)
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': True})
        self.assertEqual(s['grants']['g1']['sale']['delivered'], 100000)
        self.assertEqual(s['wallets']['alice'], 70000)                         # nothing extra yet
        s = apply(s, 'advance', {'actor': 'alice', 'days': 30})
        s = apply(s, 'claim', {'actor': 'lena.lender', 'grant': 'g1'})          # the buyer can trigger it
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't2', 'ok': True})
        self.assertIsNone(s['grants']['g1']['sale'])
        self.assertEqual(s['wallets']['alice'], 70000 + 10000)

    def test_failed_transfer_restores_and_never_double_delivers(self):
        s = sold(initial())
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        self.assertEqual(card(s)['sale']['reserved'], 50000)
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': False})
        self.assertEqual(card(s)['sale']['delivered'], 0)
        self.assertEqual(card(s)['sale']['reserved'], 0)
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't2', 'ok': True})
        self.assertEqual(s['wallets']['lena.lender'], 800000 - 30000 + 50000)

    def test_one_commitment_per_grant(self):
        s = sold(initial())
        for action, data in [('borrow', {'amount': 1000}), ('stake', {}), ('sell', {'slice': 10, 'price': 5})]:
            with self.assertRaises(Rejected):
                apply(s, action, dict(data, actor='alice', grant='g1'))
        self.assertFalse(card(s)['options']['sell']['available'])
        self.assertFalse(card(s)['options']['borrow']['available'])

    def test_limits_and_wrong_callers(self):
        s = initial()
        bad = [{'actor': 'alice', 'grant': 'g2', 'slice': 10, 'price': 5},         # cancellable grant
               {'actor': 'alice', 'grant': 'g1', 'slice': 365001, 'price': 5},     # above unvested
               {'actor': 'alice', 'grant': 'g1', 'slice': 100, 'price': 101},      # price above slice
               {'actor': 'lena.lender', 'grant': 'g1', 'slice': 100, 'price': 50}]  # not the holder
        for d in bad:
            with self.assertRaises(Rejected):
                apply(s, 'sell', d)
        self.assertEqual(s, initial())

    def test_cancel_after_sale_shows_shortfall(self):
        s = initial()
        s['grants']['g2']['cancellable'] = False
        s = apply(s, 'sell', {'actor': 'alice', 'grant': 'g2', 'slice': 80000, 'price': 40000})
        s['grants']['g2']['cancellable'] = True
        s = apply(s, 'advance', {'actor': 'alice', 'days': 60})                # 40k vested
        s = apply(s, 'cancel', {'actor': 'dao.treasury', 'grant': 'g2'})
        self.assertEqual(s['grants']['g2']['sale']['shortfall'], 40000)


class StakeAndEarn(unittest.TestCase):
    def test_rewards_accrue_on_escrow_and_mint_on_collect(self):
        s = apply(initial(), 'stake', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 365})
        expected = 365000 * loan.STAKE_BP // 10000
        self.assertEqual(s['grants']['g1']['stake']['unpaid'], expected)
        supply0 = s['supply']
        s = apply(s, 'collect', {'actor': 'alice', 'grant': 'g1'})
        self.assertEqual(s['wallets']['alice'], expected)
        self.assertEqual(s['supply'], supply0 + expected)
        with self.assertRaises(Rejected):
            apply(s, 'collect', {'actor': 'alice', 'grant': 'g1'})

    def test_holder_keeps_every_unlock_while_staked(self):
        s = apply(initial(), 'stake', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 100})
        s = apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': 't1', 'ok': True})
        self.assertEqual(s['wallets']['alice'], 100000)
        s = apply(s, 'advance', {'actor': 'alice', 'days': 10})                # earns only on what is left
        self.assertEqual(s['grants']['g1']['stake']['unpaid'],
                         365000 * loan.STAKE_BP * 100 // (10000 * 365) + 265000 * loan.STAKE_BP * 10 // (10000 * 365))

    def test_stake_blocks_loan_and_sale_until_unstaked(self):
        s = apply(initial(), 'stake', {'actor': 'alice', 'grant': 'g1'})
        with self.assertRaises(Rejected):
            apply(s, 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 1000})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 30})
        with self.assertRaises(Rejected):
            apply(s, 'unstake', {'actor': 'alice', 'grant': 'g1'})              # collect first
        s = apply(s, 'collect', {'actor': 'alice', 'grant': 'g1'})
        s = apply(s, 'unstake', {'actor': 'alice', 'grant': 'g1'})
        self.assertTrue(card(s)['options']['borrow']['available'])

    def test_cancel_ends_stake_and_pays_earned_rewards(self):
        s = apply(initial(), 'stake', {'actor': 'alice', 'grant': 'g2'})
        s = apply(s, 'advance', {'actor': 'alice', 'days': 60})
        earned = s['grants']['g2']['stake']['unpaid']
        self.assertGreater(earned, 0)
        s = apply(s, 'cancel', {'actor': 'dao.treasury', 'grant': 'g2'})
        self.assertIsNone(s['grants']['g2']['stake'])
        self.assertEqual(s['wallets']['alice'], earned)


if __name__ == '__main__':
    unittest.main()
