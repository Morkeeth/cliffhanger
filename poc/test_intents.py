"""Dry NEAR Intents plans for a real choice, including vested AURORA. No network."""
import json, os, subprocess, sys, unittest
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

    def test_settlement_is_one_non_dry_route_and_a_wallet_transaction(self):
        sized = plan('g5', 'sell')
        self.assertTrue(all(req['body']['dry'] is True for req in sized['requests']))
        settled = intents.settle_request(sized, 'USDC on Base', 'holder.near', '0xabcDEFabcdefABCDEFabcdefABCDEFabcdefABCD')
        self.assertFalse(settled['body']['dry'])
        self.assertEqual(settled['body']['refundTo'], 'holder.near')
        self.assertEqual(settled['body']['recipient'], '0xabcDEFabcdefABCDEFabcdefABCDEFabcdefABCD')
        self.assertEqual(settled['body']['originAsset'], intents.WRAP_NEAR)
        self.assertEqual(settled['kind'], 'near')
        near = intents.settle_request(sized, 'AURORA on NEAR', 'Holder.Near', '')
        self.assertEqual(near['body']['recipient'], 'holder.near')
        tx = intents.deposit_transaction('near', 'holder.near', 'ab' * 32, settled['amount'])
        self.assertEqual([a['params']['methodName'] for a in tx['actions']],
                         ['storage_deposit', 'near_deposit', 'ft_transfer'])
        self.assertEqual(tx['receiverId'], 'wrap.near')
        self.assertEqual(tx['actions'][1]['params']['deposit'], settled['amount'])
        self.assertEqual(tx['actions'][2]['params']['args']['amount'], settled['amount'])
        self.assertEqual(tx['actions'][2]['params']['deposit'], '1')
        aurora = plan('aur3', 'claim')
        sold = intents.settle_request(aurora, 'wNEAR', 'holder.near', '')
        self.assertEqual(sold['kind'], 'aurora')
        self.assertFalse(sold['body']['dry'])
        atx = intents.deposit_transaction('aurora', 'holder.near', 'cd' * 32, sold['amount'])
        self.assertEqual([a['params']['methodName'] for a in atx['actions']],
                         ['storage_deposit', 'ft_transfer'])
        self.assertEqual(atx['receiverId'], intents.AURORA_CONTRACT)
        self.assertNotIn('near_deposit', [a['params']['methodName'] for a in atx['actions']])
        with self.assertRaises(ValueError):
            intents.settle_request(sized, 'USDC on Base', 'holder.near', intents.PLACEHOLDER_EVM)
        with self.assertRaises(ValueError):
            intents.settle_request(sized, 'USDC on Base', 'not an account', '0xabcDEFabcdefABCDEFabcdefABCDEFabcdefABCD')
        with self.assertRaises(ValueError):
            intents.deposit_transaction('near', 'holder.near', 'ab' * 32, '0')
        # Bytes MyNearWallet's /sign page accepts, checked against @near-js/transactions.
        fixture = intents.deposit_transaction('near', 'holder.near', 'ab' * 32, '1000000000000000000000000')
        near_bytes = intents.transaction_base64(fixture, 'ed25519:' + '1' * 32, 7, '1' * 32)
        self.assertEqual(near_bytes, (
            'CwAAAGhvbGRlci5uZWFyAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABwAAAAAAAAAJAAAAd3JhcC5uZWFy'
            'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADAAAAAg8AAABzdG9yYWdlX2RlcG9zaXRRAAAAeyJhY2NvdW50X2lkIjoi'
            'YWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYiJ9AOBX60gbAAAA'
            'AEhWNxk8w0MAAAAAAAAAAgwAAABuZWFyX2RlcG9zaXQCAAAAe30A4FfrSBsAAAAAAKHtzM4bwtMAAAAAAAACCwAAAGZ0X3RyYW5z'
            'ZmVyjAAAAHsicmVjZWl2ZXJfaWQiOiJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFiYWJhYmFi'
            'YWJhYmFiYWJhYmFiIiwiYW1vdW50IjoiMTAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMCIsIm1lbW8iOiJjbGlmZmhhbmdlciJ9AOBX'
            '60gbAAABAAAAAAAAAAAAAAAAAAAA'))
        aurora_tx = intents.deposit_transaction('aurora', 'alice.near', 'cd' * 32, '42')
        aurora_bytes = intents.transaction_base64(
            aurora_tx, 'ed25519:2K6s7H7yQ7d8s2oA4m1pZ9cV3bN5xL8tR6uW1qY4eH7j', 99,
            '9K6s7H7yQ7d8s2oA4m1pZ9cV3bN5xL8tR6uW1qY4eH7j')
        self.assertEqual(aurora_bytes, (
            'CgAAAGFsaWNlLm5lYXIAE37FmMVK3Bilnke5M9mBJFVS/k44Hmwn3ek6hAj0dh5jAAAAAAAAADwAAABhYWFhYWEyMGQ5ZTBlMjQ2'
            'MTY5Nzc4MmVmMTE2NzVmNjY4MjA3OTYxLmZhY3RvcnkuYnJpZGdlLm5lYXJ7gJSRm9JdxIA/2HCVsps2by5TjvGiz7iNu1KECPR2'
            'HgIAAAACDwAAAHN0b3JhZ2VfZGVwb3NpdFEAAAB7ImFjY291bnRfaWQiOiJjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2Rj'
            'ZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkIn0A4FfrSBsAAAAASFY3GTzDQwAAAAAAAAACCwAAAGZ0X3RyYW5zZmVy'
            'dQAAAHsicmVjZWl2ZXJfaWQiOiJjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2RjZGNkY2Rj'
            'ZGNkY2RjZGNkIiwiYW1vdW50IjoiNDIiLCJtZW1vIjoiY2xpZmZoYW5nZXIifQDgV+tIGwAAAQAAAAAAAAAAAAAAAAAAAA=='))

    def test_pages_quote_the_term_and_can_prepare_a_deposit(self):
        for name in ('web/index.html', 'docs/index.html'):
            with open(os.path.join(ROOT, name), encoding='utf-8') as handle:
                text = handle.read()
            self.assertIn('term_days', text)
            self.assertIn('aur3', text)
            self.assertIn(intents.AURORA_NEAR, text)
            self.assertIn('10n**18n', text)
            self.assertIn('dry:false', text)
            self.assertIn('near_deposit', text)
            self.assertIn('ft_transfer', text)
            self.assertIn('/v0/status', text)
            self.assertIn('view_access_key_list', text)
            self.assertIn('function borshTx', text)
            self.assertNotIn('every slice quote uses that short-term band', text)
        with open(os.path.join(ROOT, 'poc', 'index.html'), encoding='utf-8') as handle:
            demo = handle.read()
        self.assertIn('/api/intent-plan', demo)
        self.assertIn('/api/intent-settle', demo)
        self.assertIn('near_deposit', demo)
        self.assertIn('ft_transfer', demo)
        self.assertIn('view_access_key_list', demo)
        self.assertIn('function borshTx', demo)
        self.assertNotIn('Sell a slice at 40% off', demo)
        blocks = []
        for name in ('web/index.html', 'docs/index.html', 'poc/index.html'):
            with open(os.path.join(ROOT, name), encoding='utf-8') as handle:
                text = handle.read()
            start = text.index('/* borsh-tx */')
            end = text.index('/* /borsh-tx */')
            blocks.append(text[start:end])
        self.assertEqual(len(set(blocks)), 1)
        tx = intents.deposit_transaction('near', 'holder.near', 'ab' * 32, '1000000000000000000000000')
        script = blocks[0] + '\nconsole.log(borshTx(%s, %s, 7, %s));\n' % (
            json.dumps(tx), json.dumps('ed25519:' + '1' * 32), json.dumps('1' * 32))
        got = subprocess.run(['node', '--input-type=module', '-e', script], check=True, capture_output=True, text=True)
        self.assertEqual(got.stdout.strip(), intents.transaction_base64(tx, 'ed25519:' + '1' * 32, 7, '1' * 32))
        aurora_tx = intents.deposit_transaction('aurora', 'alice.near', 'cd' * 32, '42')
        script = blocks[0] + '\nconsole.log(borshTx(%s, %s, 99, %s));\n' % (
            json.dumps(aurora_tx), json.dumps('ed25519:2K6s7H7yQ7d8s2oA4m1pZ9cV3bN5xL8tR6uW1qY4eH7j'),
            json.dumps('9K6s7H7yQ7d8s2oA4m1pZ9cV3bN5xL8tR6uW1qY4eH7j'))
        got = subprocess.run(['node', '--input-type=module', '-e', script], check=True, capture_output=True, text=True)
        self.assertEqual(got.stdout.strip(), intents.transaction_base64(
            aurora_tx, 'ed25519:2K6s7H7yQ7d8s2oA4m1pZ9cV3bN5xL8tR6uW1qY4eH7j', 99,
            '9K6s7H7yQ7d8s2oA4m1pZ9cV3bN5xL8tR6uW1qY4eH7j'))


if __name__ == '__main__':
    unittest.main()
