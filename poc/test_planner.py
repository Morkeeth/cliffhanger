import unittest
from planner import plan, offx_discount, SOURCED, DEMO, USER

POS = {'locked': 120_000, 'price_now': 2.0, 'months_left': 12, 'cliff_months_left': 0, 'cancellable': False}


class Planner(unittest.TestCase):
    def test_every_row_sums(self):
        p = plan(POS)
        for chs in p['holder'].values():
            for r in chs.values():
                self.assertAlmostEqual(r['total'], r['cash_now'] + r['kept_value'] + r['rewards_value'], places=2)

    def test_every_assumption_is_labelled_with_a_source(self):
        for k, a in plan(POS)['assumptions'].items():
            self.assertIn(a['label'], (SOURCED, DEMO, USER), k)
            self.assertTrue(a['source'], k)
        self.assertEqual(plan(POS)['assumptions']['sale_discount']['label'], SOURCED)
        self.assertEqual(plan(POS, {'stake_yield': 0.03})['assumptions']['stake_yield']['label'], USER)

    def test_worst_case_is_the_true_minimum(self):
        p = plan(POS)
        lowest = min(r['total'] for chs in p['holder'].values() for r in chs.values())
        self.assertEqual(p['worst_holder']['total'], lowest)
        self.assertEqual(p['worst_holder']['scenario'], 'down')
        fl = min(x['gain'] for chs in p['funder'].values() for x in chs.values())
        self.assertEqual(p['worst_funder']['gain'], fl)

    def test_sale_numbers_follow_offx_and_mirror_the_buyer(self):
        p = plan(POS)
        flat = p['holder']['flat']
        self.assertEqual(flat['sell']['cash_now'], 30_000 * 0.6 * 2.0)
        self.assertEqual(flat['sell']['vs_hold'], -30_000 * 0.4 * 2.0)
        buyer = p['funder']['flat']['buy_slice']
        self.assertAlmostEqual(buyer['gain'], -flat['sell']['vs_hold'], places=2)

    def test_lender_gain_mirrors_holder_cost(self):
        p = plan(POS)
        for s in ('down', 'flat', 'up'):
            h, l = p['holder'][s], p['funder'][s]['lend']
            self.assertAlmostEqual(l['gain'], -h['borrow']['vs_hold'], places=1)
        fee_tokens = 48_000 * 0.12 * 1
        self.assertAlmostEqual(p['holder']['flat']['borrow']['vs_hold'], -fee_tokens * 2.0, places=2)

    def test_stake_beats_hold_and_rewards_fall_as_escrow_unlocks(self):
        p = plan(POS)
        self.assertGreater(p['holder']['flat']['stake']['vs_hold'], 0)
        full = 120_000 * 0.045 * 2.0
        self.assertLess(p['holder']['flat']['stake']['rewards_value'], full)
        cliff = plan(dict(POS, cliff_months_left=12))
        self.assertAlmostEqual(cliff['holder']['flat']['stake']['rewards_value'], full, places=2)

    def test_stake_rewards_match_the_engine_when_claiming_monthly(self):
        import loan
        s = loan.apply(loan.initial(), 'stake', {'actor': 'alice', 'grant': 'g1'})
        day = 0
        for m in range(1, 13):
            nd = min(round(m * 365 / 12), 365)
            s = loan.apply(s, 'advance', {'actor': 'alice', 'days': nd - day})
            day = nd
            if loan.claimable(s['grants']['g1'], day) > 0:
                s = loan.apply(s, 'claim', {'actor': 'alice', 'grant': 'g1'})
                s = loan.apply(s, 'resolve', {'actor': 'alice', 'grant': 'g1', 'transfer': s['transfers'][-1]['id'], 'ok': True})
        st = s['grants']['g1']['stake']
        engine = st['unpaid'] + st['paid']
        p = plan({'locked': 365_000, 'price_now': 1.0, 'months_left': 12, 'cliff_months_left': 3, 'cancellable': False})
        self.assertAlmostEqual(p['holder']['flat']['stake']['rewards_value'], engine, delta=engine * 0.01)

    def test_cancellable_grant_shows_only_hold_and_stake(self):
        p = plan(dict(POS, cancellable=True))
        self.assertEqual(set(p['holder']['flat']), {'hold', 'stake'})
        self.assertTrue(p['unavailable'])
        self.assertEqual(p['funder']['flat'], {})

    def test_offx_gap_needs_user_value(self):
        self.assertIsNone(offx_discount(30))
        with self.assertRaises(ValueError):
            plan(dict(POS, months_left=30))
        p = plan(dict(POS, months_left=30), {'sale_discount': 0.55})
        self.assertEqual(p['assumptions']['sale_discount']['label'], USER)
        self.assertEqual(offx_discount(24), 0.50)
        self.assertEqual(offx_discount(36), 0.60)

    def test_bad_position_rejected(self):
        for bad in ({'locked': 0}, {'price_now': 0}, {'months_left': 0}):
            with self.assertRaises(ValueError):
                plan(dict(POS, **bad))


if __name__ == '__main__':
    unittest.main()
