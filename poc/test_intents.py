"""Dry NEAR Intents plans for a real choice, including vested AURORA. No network."""
import os, sys, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import intents
import loan
import server
from loan import initial, options


ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def plan(grant, choice, **q):
    query = {'grant': grant, 'choice': choice, 'price': '0.0001'}
    query.update(q)
    return server.intent_plan(initial(), query)


class IntentPlans(unittest.TestCase):
    def test_long_grant_uses_the_fifty_percent_band(self):
        card = next(o for o in options(initial(), 'alice') if o['id'] == 'g5')
        self.assertEqual(card['options']['sell']['months_left'], 24)
        self.assertEqual(card['options']['sell']['discount'], 0.50)
        got = plan('g5', 'sell')
        self.assertEqual(got['slice'], 125000)
        self.assertEqual(got['tokens'], 62500)
        self.assertEqual(got['discount_pct'], 50)
        self.assertEqual(got['origin'], intents.WRAP_NEAR)
        self.assertTrue(got['near'].startswith('6.25'))
        self.assertTrue(all(req['body']['dry'] is True for req in got['requests']))
        self.assertEqual([req['body']['destinationAsset'] for req in got['requests']],
                         [intents.AURORA_NEAR, intents.AURORA_ETH, intents.USDC_BASE])

    def test_short_grant_stays_at_forty_percent(self):
        card = next(o for o in options(initial(), 'alice') if o['id'] == 'g1')
        self.assertEqual(card['options']['sell']['months_left'], 12)
        self.assertEqual(card['options']['sell']['discount'], 0.40)
        got = plan('g1', 'sell')
        self.assertEqual(got['slice'], 91250)
        self.assertEqual(got['tokens'], 54750)

    def test_borrow_and_claim_follow_the_engine(self):
        borrowed = plan('g1', 'borrow')
        self.assertEqual(borrowed['tokens'], 146000)
        self.assertEqual(borrowed['origin'], intents.WRAP_NEAR)
        claimed = plan('g4', 'claim')
        self.assertEqual(claimed['tokens'], 80000)
        self.assertIn('already vested', claimed['note'])
        with self.assertRaises(ValueError):
            plan('g4', 'sell')
        with self.assertRaises(ValueError):
            plan('g2', 'sell')

    def test_custom_slice_and_borrow_amount_are_the_ones_quoted(self):
        got = plan('g3', 'sell', slice='1000')
        self.assertEqual(got['slice'], 1000)
        self.assertEqual(got['tokens'], 600)  # 7 months, 40% off
        got = plan('g1', 'borrow', amount='1000')
        self.assertEqual(got['tokens'], 1000)
        with self.assertRaises(ValueError):
            plan('g1', 'borrow', amount='146001')

    def test_vested_aurora_is_quoted_as_itself(self):
        cards = {c['id']: c for c in intents.aurora_cards()}
        self.assertEqual(cards['aur1']['vested'], 400000)
        self.assertEqual(cards['aur1']['unvested'], 600000)
        self.assertEqual(cards['aur2']['vested'], 200000)
        self.assertEqual(cards['aur3']['vested'], 80000)
        self.assertEqual(cards['aur3']['unvested'], 0)
        self.assertFalse(cards['aur1']['ledger'])
        sold = plan('aur1', 'sell')
        self.assertEqual(sold['discount_pct'], 50)
        self.assertEqual(sold['tokens'], 75000)
        self.assertEqual(sold['origin'], intents.AURORA_NEAR)
        self.assertEqual(sold['amount'], str(75000 * 10 ** 18))
        self.assertEqual(sold['requests'][0]['body']['destinationAsset'], intents.WRAP_NEAR)
        self.assertTrue(all(req['body']['dry'] is True for req in sold['requests']))
        claimed = plan('aur3', 'claim')
        self.assertEqual(claimed['tokens'], 80000)
        self.assertEqual(claimed['origin'], intents.AURORA_NEAR)
        with self.assertRaises(ValueError):
            plan('aur3', 'borrow')

    def test_desk_keeps_locked_aurora_and_drops_the_fully_vested_one(self):
        rows = {row['id']: row for row in server.desk_data(initial())}
        self.assertEqual(rows['aur1']['pos']['locked'], 600000)
        self.assertEqual(rows['aur1']['pos']['vested'], 400000)
        self.assertEqual(rows['aur2']['pos']['vested'], 200000)
        self.assertNotIn('aur3', rows)
        self.assertNotIn('g4', rows)

    def test_gap_in_the_offx_bands_needs_an_entered_discount(self):
        pos = {'token': 'CLIFF', 'title': 'gap', 'unvested': 1000, 'claimable': 0,
               'max_advance': 400, 'why_not': None, 'cancellable': False, 'months_left': 30}
        with self.assertRaises(ValueError):
            intents.plan_position(pos, 'sell', price='0.0001')
        got = intents.plan_position(pos, 'sell', price='0.0001', entered_discount='0.55')
        self.assertEqual(got['discount'], 0.55)
        self.assertEqual(got['tokens'], 112)  # 250 slice, 45% of it kept

    def test_pages_quote_the_term_and_the_aurora_token(self):
        for name in ('web/index.html', 'docs/index.html'):
            with open(os.path.join(ROOT, name), encoding='utf-8') as handle:
                text = handle.read()
            self.assertIn('term_days', text)
            self.assertIn('aur3', text)
            self.assertIn(intents.AURORA_NEAR, text)
            self.assertIn('10n**18n', text)
            self.assertNotIn('every slice quote uses that short-term band', text)
        with open(os.path.join(ROOT, 'poc', 'index.html'), encoding='utf-8') as handle:
            demo = handle.read()
        self.assertIn('/api/intent-plan', demo)
        self.assertNotIn('Sell a slice at 40% off', demo)



class ReviewFixes(unittest.TestCase):
    def test_sale_quote_refused_after_borrow(self):
        import loan, server
        s = loan.apply(loan.initial(), 'borrow', {'actor': 'alice', 'grant': 'g1', 'amount': 1000})
        with self.assertRaisesRegex(ValueError, 'already committed'):
            server.intent_plan(s, {'grant': 'g1', 'choice': 'sell', 'price': '0.0001'})

    def test_blank_discount_is_not_zero(self):
        for blank in ('', '  '):
            with self.assertRaisesRegex(ValueError, 'enter a discount'):
                intents.discount_for(30, blank)


if __name__ == '__main__':
    unittest.main()
