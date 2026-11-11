"""Size one Cliffhanger choice and the dry NEAR Intents requests that quote it.

Every request is dry. Nothing in this module sends a swap or talks to the network.
Aurora's own chain is not in the live 1Click token list. The AURORA token is, on NEAR
and on Ethereum, and that is the Aurora route these requests use.
"""
import math
from decimal import Decimal, ROUND_DOWN

from planner import offx_discount

ONECLICK = 'https://1click.chaindefuser.com/v0/quote'
WRAP_NEAR = 'nep141:wrap.near'
AURORA_NEAR = 'nep141:aaaaaa20d9e0e2461697782ef11675f668207961.factory.bridge.near'
AURORA_ETH = 'nep141:eth-0xaaaaaa20d9e0e2461697782ef11675f668207961.omft.near'
USDC_BASE = 'nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near'
AURORA_DECIMALS = 18
NEAR_DECIMALS = 24
PLACEHOLDER_NEAR = 'placeholder.near'
PLACEHOLDER_EVM = '0x1111111111111111111111111111111111111111'
# Same 0.04 the treasury desk labels as a demo figure near the Intents price. Not a live feed.
AURORA_DEMO_PRICE = 0.04

# Second token, with vesting. Not part of the CLIFF ledger: claiming these must not
# credit alice's CLIFF balance. aur1 and aur2 still have a locked remainder, so the
# desk can plan them. aur3 is fully vested, so only the action page quotes it.
AURORA_GRANTS = [
    {'id': 'aur1', 'title': 'Aurora ecosystem grant, already vesting', 'token': 'AURORA',
     'total': 1_000_000, 'vested': 400_000, 'claimable': 400_000, 'unvested': 600_000,
     'months_left': 18, 'cliff_months_left': 0, 'cancellable': False},
    {'id': 'aur2', 'title': 'Aurora contributor, half the term left', 'token': 'AURORA',
     'total': 400_000, 'vested': 200_000, 'claimable': 200_000, 'unvested': 200_000,
     'months_left': 9, 'cliff_months_left': 0, 'cancellable': False},
    {'id': 'aur3', 'title': 'Aurora grant, fully vested', 'token': 'AURORA',
     'total': 80_000, 'vested': 80_000, 'claimable': 80_000, 'unvested': 0,
     'months_left': 0, 'cliff_months_left': 0, 'cancellable': False},
]


def months_left(term_days):
    """Same month count the treasury desk uses: 30.42 days, rounded up, at least 1 while time remains."""
    if term_days <= 0:
        return 0
    return max(1, math.ceil(term_days / 30.42))


def discount_for(months, entered=None):
    """OFFX band for the months still left. 25 to 35 months has no band, so the caller enters one."""
    if months <= 0:
        return None
    band = offx_discount(months)
    if band is not None:
        return band
    if entered is None:
        raise ValueError('No OFFX band between 25 and 35 months; enter a discount')
    entered = float(entered)
    if not 0 <= entered < 1:
        raise ValueError('Discount must be at least 0 and below 1')
    return entered


def _sale(locked, months, slice_tokens, entered):
    if locked <= 0:
        raise ValueError('Nothing locked can be sold')
    sl = locked // 4 if slice_tokens is None else int(slice_tokens)
    if sl <= 0 or sl > locked:
        raise ValueError('Slice must be within what is still locked')
    discount = discount_for(months, entered)
    pct = round(discount * 100)
    cash = sl * (100 - pct) // 100
    if cash <= 0:
        raise ValueError('The sale proceeds are zero')
    return sl, discount, pct, cash


def _units_aurora(tokens):
    return str(int(tokens) * 10 ** AURORA_DECIMALS)


def _units_near(tokens, price):
    if price is None or str(price).strip() == '':
        raise ValueError('Enter a NEAR price per CLIFF above zero')
    near = (Decimal(int(tokens)) * Decimal(str(price))).quantize(Decimal('1e-8'), rounding=ROUND_DOWN)
    if near <= 0:
        raise ValueError('The NEAR amount is zero')
    return str(int(near * (Decimal(10) ** NEAR_DECIMALS))), format(near, 'f')


def _body(origin, dest, amount, recipient):
    return {
        'dry': True,
        'swapType': 'EXACT_INPUT',
        'slippageTolerance': 100,
        'originAsset': origin,
        'depositType': 'ORIGIN_CHAIN',
        'destinationAsset': dest,
        'amount': amount,
        'refundTo': PLACEHOLDER_NEAR,
        'refundType': 'ORIGIN_CHAIN',
        'recipient': recipient,
        'recipientType': 'DESTINATION_CHAIN',
    }


def _routes(token, amount):
    """CLIFF has no market, so the holder names a NEAR price and the quote spends that NEAR.
    AURORA is listed, so the quote spends the AURORA the choice pays."""
    if token == 'AURORA':
        specs = [
            ('wNEAR', WRAP_NEAR, PLACEHOLDER_NEAR),
            ('AURORA on Ethereum', AURORA_ETH, PLACEHOLDER_EVM),
            ('USDC on Base', USDC_BASE, PLACEHOLDER_EVM),
        ]
        origin = AURORA_NEAR
    elif token == 'CLIFF':
        specs = [
            ('AURORA on NEAR', AURORA_NEAR, PLACEHOLDER_NEAR),
            ('AURORA on Ethereum', AURORA_ETH, PLACEHOLDER_EVM),
            ('USDC on Base', USDC_BASE, PLACEHOLDER_EVM),
        ]
        origin = WRAP_NEAR
    else:
        raise ValueError('No Intents route for this token')
    requests = [{'label': label, 'body': _body(origin, dest, amount, who)} for label, dest, who in specs]
    return origin, requests


def plan_position(pos, choice, price=None, amount=None, slice_tokens=None, entered_discount=None):
    """pos: token, title, unvested, claimable, max_advance, months_left, cancellable, why_not.
    Returns the sized choice and one dry request per route. Does not call 1Click."""
    token = pos['token']
    kind = choice
    months = pos.get('months_left') or 0
    if kind == 'borrow':
        if not pos.get('max_advance'):
            raise ValueError(pos.get('why_not') or 'This grant cannot be borrowed against')
        tokens = pos['max_advance'] if amount is None else int(amount)
        if tokens <= 0 or tokens > pos['max_advance']:
            raise ValueError('Outside the borrow limit')
        note = 'Borrow %s %s.' % (f'{tokens:,}', token)
        discount, pct, sl = None, None, None
    elif kind == 'sell':
        if pos.get('cancellable') or not pos.get('unvested'):
            raise ValueError('A cancellable grant cannot be sold' if pos.get('cancellable') else 'Nothing locked can be sold')
        sl, discount, pct, tokens = _sale(pos['unvested'], months, slice_tokens, entered_discount)
        note = 'Sell %s %s. At %d%% off (%d months left) the holder receives %s %s.' % (
            f'{sl:,}', token, pct, months, f'{tokens:,}', token)
    elif kind == 'claim':
        tokens = pos.get('claimable') or 0
        if tokens <= 0:
            raise ValueError('Nothing has vested yet on this grant')
        note = 'Cash the %s %s that have already vested.' % (f'{tokens:,}', token)
        discount, pct, sl = None, None, None
    else:
        raise ValueError('Choice must be borrow, sell or claim')

    if token == 'AURORA':
        amount_units = _units_aurora(tokens)
        near_text = None
        note += ' NEAR Intents prices the AURORA token. Dry quotes only.'
    else:
        amount_units, near_text = _units_near(tokens, price)
        note += ' At your price that is %s NEAR. Dry quotes only.' % near_text
    origin, requests = _routes(token, amount_units)
    if any(not req['body'].get('dry') for req in requests):
        raise RuntimeError('Intents plan produced a request that is not dry')
    return {
        'dry': True,
        'endpoint': ONECLICK,
        'token': token,
        'choice': kind,
        'tokens': tokens,
        'slice': sl,
        'discount': discount,
        'discount_pct': pct,
        'months_left': months,
        'origin': origin,
        'amount': amount_units,
        'near': near_text,
        'note': note,
        'requests': requests,
    }


def aurora_cards():
    """Action-page rows for the AURORA schedules. ledger is false: the CLIFF engine does not hold them."""
    cards = []
    for g in AURORA_GRANTS:
        months = g['months_left']
        discount = offx_discount(months) if months else None
        max_advance = 0 if g['cancellable'] or g['unvested'] <= 0 else g['unvested'] * 40 // 100
        why = None if max_advance > 0 else (
            'cancellable grants cannot back a loan' if g['cancellable'] else 'nothing left unvested')
        can_sell = (not g['cancellable']) and g['unvested'] > 0
        cards.append({
            'id': g['id'], 'title': g['title'], 'token': 'AURORA', 'ledger': False,
            'total': g['total'], 'vested': g['vested'], 'claimable': g['claimable'],
            'unvested': g['unvested'],
            'next_unlock': 'fully vested' if g['unvested'] == 0 else 'vesting daily',
            'cancellable': g['cancellable'], 'cancelled': False, 'lien': None, 'sale': None,
            'options': {
                'claim': g['claimable'] > 0,
                'borrow': {'available': max_advance > 0, 'max_advance': max_advance,
                           'ltv_pct': 40 if max_advance else 0, 'why_not': why},
                'repay': False,
                'sell': {'available': can_sell, 'max_slice': g['unvested'] if can_sell else 0,
                         'discount': discount, 'months_left': months},
                'stake': {'available': False, 'yield_bp': 0, 'collect': False},
            },
        })
    return cards


def position_from_card(card):
    sell = card['options']['sell']
    borrow = card['options']['borrow']
    return {
        'token': card['token'], 'title': card['title'],
        'unvested': card['unvested'], 'claimable': card['claimable'],
        'max_advance': borrow['max_advance'], 'why_not': borrow['why_not'],
        'cancellable': card['cancellable'], 'months_left': sell['months_left'],
    }
