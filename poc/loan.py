"""NEAR vesting-loan POC engine. Local demo state only: no RPC, wallet, chain or real value.

Method names mirror the planned near-sdk contract (PLAN.md slice 1) so the Rust port is a
translation, not a redesign. Every action copies state first; a rejected action changes nothing.
"""
from copy import deepcopy

from intents import months_left
from planner import offx_discount

LTV_FIXED = 40          # percent of unvested value a non-cancellable grant can borrow
LTV_CANCELLABLE = 0     # cancellable grants can vanish, so they cannot back a loan
APR = 12                # demo flat annual fee, percent; not a market price
STAKE_BP = 450          # demo staking yield, basis points a year (about 4.5%, reported after NEAR's Oct 2025 inflation cut); a planner input, not a promise


class Rejected(ValueError):
    pass


def initial():
    # g3 and g4 start before day 0 so the demo opens with tokens already vested.
    # Supply covers the original two grants plus these three. Units have no value.
    supply = 2_820_000
    s = {
        'day': 0,
        'labels': {'note': 'DEMO STATE: seeded local accounts, units have no value'},
        'wallets': {'dao.treasury': 2_020_000, 'alice': 0, 'lena.lender': 800_000},
        'grants': {
            'g1': {'title': 'Core contributor grant', 'token': 'CLIFF', 'issuer': 'dao.treasury', 'holder': 'alice',
                   'total': 365_000, 'start': 0, 'cliff': 90, 'duration': 365,
                   'cancellable': False, 'cancelled_at': None, 'escrow': 0, 'claimed': 0,
                   'pending': 0, 'lien': None},
            'g2': {'title': 'Advisor grant (cancellable)', 'token': 'CLIFF', 'issuer': 'dao.treasury', 'holder': 'alice',
                   'total': 120_000, 'start': 0, 'cliff': 30, 'duration': 180,
                   'cancellable': True, 'cancelled_at': None, 'escrow': 0, 'claimed': 0,
                   'pending': 0, 'lien': None},
            'g3': {'title': 'Engineer grant, already vesting', 'token': 'CLIFF', 'issuer': 'dao.treasury', 'holder': 'alice',
                   'total': 240_000, 'start': -180, 'cliff': 30, 'duration': 365,
                   'cancellable': False, 'cancelled_at': None, 'escrow': 0, 'claimed': 0,
                   'pending': 0, 'lien': None},
            'g4': {'title': 'Finished grant, fully vested', 'token': 'CLIFF', 'issuer': 'dao.treasury', 'holder': 'alice',
                   'total': 80_000, 'start': -400, 'cliff': 0, 'duration': 365,
                   'cancellable': False, 'cancelled_at': None, 'escrow': 0, 'claimed': 0,
                   'pending': 0, 'lien': None},
            'g5': {'title': 'Hire still inside the cliff', 'token': 'CLIFF', 'issuer': 'dao.treasury', 'holder': 'alice',
                   'total': 500_000, 'start': 0, 'cliff': 180, 'duration': 730,
                   'cancellable': False, 'cancelled_at': None, 'escrow': 0, 'claimed': 0,
                   'pending': 0, 'lien': None},
        },
        'transfers': [], 'events': [], 'supply': supply,
    }
    for g in s['grants'].values():          # issuer funds escrow up front
        s['wallets'][g['issuer']] -= g['total']
        g['escrow'] = g['total']
    return s


# ---------- views ----------

def vested(g, day):
    end = g['cancelled_at'] if g['cancelled_at'] is not None else day
    t = min(day, end) - g['start']
    if t < g['cliff']:
        return 0
    return g['total'] * min(t, g['duration']) // g['duration']


def unvested(g, day):
    if g['cancelled_at'] is not None:
        return 0
    return g['total'] - vested(g, day)


def encumbrance(g):
    """The one thing a grant is committed to: a loan, a sold slice or a stake. Never two."""
    return g['lien'] or g.get('sale') or g.get('stake')


def owed(g):
    """Tokens the grant still owes its counterparty from future unlocks."""
    if g['lien']:
        return g['lien']['debt']
    if g.get('sale'):
        return g['sale']['slice'] - g['sale']['delivered']
    return 0


def reserved(s, gid):
    """Amount already on its way to the lender or buyer in a pending claim on the current commitment."""
    g = s['grants'][gid]
    e = g['lien'] or g.get('sale')
    if not e:
        return 0
    return sum(a for t in s['transfers'] if t['grant'] == gid and t['state'] == 'pending'
               and t.get('lien_id') == e.get('id') for _w, a, role in t['legs'] if role in ('lender', 'buyer'))


def set_shortfall(s, gid, day):
    g = s['grants'][gid]
    e = g['lien'] or g.get('sale')
    if e:
        e['shortfall'] = (max(0, owed(g) - reserved(s, gid) - claimable(g, day))
                          if g['cancelled_at'] is not None else 0)


def accrue_stakes(s, days):
    """Staked escrow earns for the days that pass. Rewards are new tokens, paid when collected."""
    for g in s['grants'].values():
        st = g.get('stake')
        if st:
            st['unpaid'] += g['escrow'] * STAKE_BP * days // (10000 * 365)


def claimable(g, day):
    return vested(g, day) - g['claimed'] - g['pending']


def quote(g, day):
    ltv = LTV_CANCELLABLE if g['cancellable'] else LTV_FIXED
    remaining = max(0, g['start'] + g['duration'] - day)
    max_advance = 0 if encumbrance(g) or g['cancelled_at'] is not None else unvested(g, day) * ltv // 100
    return {'ltv_pct': ltv, 'max_advance': max_advance, 'term_days': remaining,
            'fee_for': lambda p: p * APR * remaining // (100 * 365)}


def options(state, account):
    """Entry screen: every grant the account holds, and what it can do with each."""
    day = state['day']
    out = []
    for gid, g in state['grants'].items():
        if g['holder'] != account:
            continue
        q = quote(g, day)
        months = months_left(q['term_days'])
        discount = offx_discount(months) if months else None
        free = not encumbrance(g) and g['cancelled_at'] is None
        if unvested(g, day) == 0 and g['cancelled_at'] is None:
            nxt = 'fully vested'
        elif day < g['start'] + g['cliff']:
            nxt = 'cliff on day %d' % (g['start'] + g['cliff'])
        else:
            nxt = 'vesting daily'
        out.append({
            'id': gid, 'title': g['title'], 'token': g.get('token', 'CLIFF'),
            'total': g['total'], 'vested': vested(g, day),
            'claimed': g['claimed'], 'claimable': claimable(g, day), 'unvested': unvested(g, day),
            'next_unlock': nxt, 'cancellable': g['cancellable'], 'cancelled': g['cancelled_at'] is not None,
            'lien': dict(g['lien'], reserved=reserved(state, gid)) if g['lien'] else None,
            'options': {
                'claim': claimable(g, day) > 0,
                'borrow': {'available': q['max_advance'] > 0, 'max_advance': q['max_advance'],
                           'ltv_pct': q['ltv_pct'], 'term_days': q['term_days'],
                           'why_not': None if q['max_advance'] > 0 else
                           ('already committed' if encumbrance(g) else
                            'cancellable grants cannot back a loan' if g['cancellable'] else
                            'nothing left unvested')},
                'repay': bool(g['lien'] and g['lien']['debt'] - reserved(state, gid) > 0),
                'sell': {'available': free and not g['cancellable'] and unvested(g, day) > 0,
                         'max_slice': unvested(g, day) if free and not g['cancellable'] else 0,
                         'discount': discount, 'months_left': months},
                'stake': {'available': free and g['escrow'] > 0, 'yield_bp': STAKE_BP,
                          'collect': bool(g.get('stake') and g['stake']['unpaid'] > 0)},
            },
            'sale': dict(g['sale'], reserved=reserved(state, gid)) if g.get('sale') else None,
            'stake': g.get('stake')})
    return out


def preview_borrow(state, gid, amount):
    g = state['grants'][gid]
    q = quote(g, state['day'])
    fee = q['fee_for'](amount)
    return {'cash_now': amount, 'fee': fee, 'repay_total': amount + fee,
            'repaid_by': 'next unlocks go to the lender first until the debt is 0',
            'if_cancelled': 'only vested-but-unclaimed tokens can repay; any shortfall is shown',
            'within_limit': 0 < amount <= q['max_advance']}


def assert_invariants(s):
    assert all(v >= 0 for v in s['wallets'].values()), 'negative wallet'
    in_flight = sum(t['amount'] for t in s['transfers'] if t['state'] == 'pending')
    assert sum(s['wallets'].values()) + sum(g['escrow'] for g in s['grants'].values()) + in_flight == s['supply'], 'conservation'
    for g in s['grants'].values():
        assert g['escrow'] >= 0 and g['pending'] >= 0
        assert g['claimed'] + g['pending'] <= max(vested(g, s['day']), g['claimed'] + g['pending'])
        assert claimable(g, s['day']) >= 0, 'over-claimed'
        if g['lien']:
            assert g['lien']['debt'] >= 0, 'negative debt'


# ---------- actions ----------

def apply(current, action, data):
    s = deepcopy(current)
    actor = data.get('actor')
    if actor not in s['wallets']:
        raise Rejected('Unknown demo account')

    def need(ok, msg):
        if not ok:
            raise Rejected(msg)

    def log(text):
        s['events'].append({'day': s['day'], 'actor': actor, 'text': text})

    if action == 'advance':
        days = data.get('days', 30)
        need(type(days) is int and 1 <= days <= 400, 'Step must be 1 to 400 days')
        accrue_stakes(s, days)
        s['day'] += days
        log('Demo clock +%d days' % days)
        assert_invariants(s)
        return s

    gid = data.get('grant')
    need(gid in s['grants'], 'Unknown grant')
    g = s['grants'][gid]
    day = s['day']

    if action == 'borrow':
        amount = data.get('amount')
        lender = data.get('lender', 'lena.lender')
        need(actor == g['holder'], 'Only the holder can pledge this grant')
        need(not encumbrance(g), 'This grant is already committed')
        need(type(amount) is int and amount > 0, 'Amount must be a positive whole number')
        q = quote(g, day)
        need(amount <= q['max_advance'], 'Above the borrow limit (%d)' % q['max_advance'])
        need(lender in s['wallets'] and lender != actor, 'Unknown lender')
        need(s['wallets'][lender] >= amount, 'Lender lacks funds')
        fee = q['fee_for'](amount)
        s['wallets'][lender] -= amount
        s['wallets'][actor] += amount
        lien_id = s.get('next_lien', 1)
        s['next_lien'] = lien_id + 1
        g['lien'] = {'id': lien_id, 'lender': lender, 'principal': amount, 'fee': fee, 'debt': amount + fee,
                     'repaid_by_unlocks': 0, 'repaid_by_holder': 0, 'opened_day': day, 'shortfall': 0}
        log('Borrowed %d against %s; owes %d' % (amount, gid, amount + fee))

    elif action == 'claim':
        need(actor in (g['holder'], g['lien']['lender'] if g['lien'] else g['sale']['buyer'] if g.get('sale') else g['holder']),
             'Only the holder, lender or buyer can trigger a claim')
        amt = claimable(g, day)
        need(amt > 0, 'Nothing claimable yet')
        need(not any(t['grant'] == gid and t['state'] == 'pending' for t in s['transfers']), 'A claim is already pending')
        e = g['lien'] or g.get('sale')
        first = min(amt, owed(g)) if e else 0
        who = g['lien']['lender'] if g['lien'] else g['sale']['buyer'] if g.get('sale') else None
        role = 'lender' if g['lien'] else 'buyer'
        # Each leg carries its role: the lender or buyer is paid first, up to what is still owed.
        legs = [x for x in ((who, first, role), (g['holder'], amt - first, 'holder')) if x[0] and x[1] > 0]
        g['escrow'] -= amt
        g['pending'] += amt
        tid = 't%d' % (len(s['transfers']) + 1)
        s['transfers'].append({'id': tid, 'grant': gid, 'amount': amt, 'legs': legs, 'lien_id': e.get('id') if e else None,
                               'state': 'pending', 'retries': 0})
        log('Claim %s pending: %s' % (tid, ', '.join('%d to %s' % (a, w) for w, a, _r in legs)))

    elif action == 'resolve':
        tid = data.get('transfer')
        t = next((x for x in s['transfers'] if x['id'] == tid and x['grant'] == gid), None)
        need(t is not None and t['state'] == 'pending', 'No such pending transfer')
        need(actor == g['holder'], 'Demo: the holder triggers the simulated callback')
        ok = data.get('ok', True)
        g['pending'] -= t['amount']
        if ok:
            for w, a, role in t['legs']:
                if role == 'lender':
                    # The claim reserved this debt, so repay could not close the lien or shrink it below a.
                    live = g['lien']
                    assert live and live['lender'] == w and live.get('id') == t.get('lien_id') \
                        and live['debt'] >= a, 'reserved lien changed'
                    live['debt'] -= a
                    live['repaid_by_unlocks'] += a
                    s['wallets'][w] += a
                elif role == 'buyer':
                    live = g.get('sale')
                    assert live and live['buyer'] == w and live.get('id') == t.get('lien_id') \
                        and live['slice'] - live['delivered'] >= a, 'reserved sale changed'
                    live['delivered'] += a
                    s['wallets'][w] += a
                else:
                    s['wallets'][w] += a
            g['claimed'] += t['amount']
            t['state'] = 'done'
            log('Transfer %s settled' % tid)
        else:
            need(t['retries'] == 0, 'Already restored once')
            g['escrow'] += t['amount']
            t['state'] = 'failed_restored'
            t['retries'] = 1
            log('Transfer %s failed; %d restored to escrow' % (tid, t['amount']))
        set_shortfall(s, gid, day)
        if g['lien'] and g['lien']['debt'] == 0:
            log('Debt cleared on %s; lien released' % gid)
            g['lien']['closed_day'] = day
            g['history'] = g.get('history', []) + [g['lien']]
            g['lien'] = None
        if g.get('sale') and g['sale']['delivered'] == g['sale']['slice']:
            log('Slice on %s fully delivered; sale closed' % gid)
            g['sale']['closed_day'] = day
            g['history'] = g.get('history', []) + [g['sale']]
            g['sale'] = None

    elif action == 'sell':
        sl, price = data.get('slice'), data.get('price')
        buyer = data.get('buyer', 'lena.lender')
        need(actor == g['holder'], 'Only the holder can sell a slice')
        need(not encumbrance(g), 'This grant is already committed')
        need(g['cancelled_at'] is None and not g['cancellable'], 'Only a grant the issuer cannot cancel can be sold')
        need(type(sl) is int and 0 < sl <= unvested(g, day), 'Slice must be 1 to %d unvested tokens' % unvested(g, day))
        need(type(price) is int and 0 < price <= sl, 'Price must be positive and at most the slice')
        need(buyer in s['wallets'] and buyer != actor and s['wallets'][buyer] >= price, 'Buyer unknown or short of funds')
        s['wallets'][buyer] -= price
        s['wallets'][actor] += price
        sid = s.get('next_lien', 1)
        s['next_lien'] = sid + 1
        g['sale'] = {'id': sid, 'buyer': buyer, 'slice': sl, 'price': price, 'delivered': 0,
                     'opened_day': day, 'shortfall': 0}
        log('Sold the next %d unlocked tokens of %s for %d' % (sl, gid, price))

    elif action == 'stake':
        need(actor == g['holder'], 'Only the holder can stake')
        need(not encumbrance(g), 'This grant is already committed')
        need(g['cancelled_at'] is None and g['escrow'] > 0, 'Nothing left to stake')
        sid = s.get('next_lien', 1)
        s['next_lien'] = sid + 1
        g['stake'] = {'id': sid, 'opened_day': day, 'yield_bp': STAKE_BP, 'unpaid': 0, 'paid': 0}
        log('Staked the escrow of %s' % gid)

    elif action == 'collect':
        need(actor == g['holder'], 'Only the holder collects rewards')
        st = g.get('stake')
        need(st is not None and st['unpaid'] > 0, 'No rewards to collect')
        r = st['unpaid']
        st['unpaid'] = 0
        st['paid'] += r
        s['wallets'][actor] += r
        s['supply'] += r
        log('Collected %d staking rewards from %s' % (r, gid))

    elif action == 'unstake':
        need(actor == g['holder'], 'Only the holder can unstake')
        st = g.get('stake')
        need(st is not None, 'Not staked')
        need(st['unpaid'] == 0, 'Collect rewards first')
        st['closed_day'] = day
        g['history'] = g.get('history', []) + [st]
        g['stake'] = None
        log('Unstaked %s' % gid)

    elif action == 'repay':
        amount = data.get('amount')
        need(actor == g['holder'], 'Only the holder repays')
        need(g['lien'] is not None and g['lien']['debt'] > 0, 'No debt to repay')
        free = g['lien']['debt'] - reserved(s, gid)
        need(free > 0, 'The pending claim already covers the rest of the debt')
        need(type(amount) is int and 0 < amount <= free, 'Repay between 1 and %d (the rest is covered by the pending claim)' % free
             if free < g['lien']['debt'] else 'Repay between 1 and the debt')
        need(s['wallets'][actor] >= amount, 'Not enough in wallet')
        s['wallets'][actor] -= amount
        s['wallets'][g['lien']['lender']] += amount
        g['lien']['debt'] -= amount
        g['lien']['repaid_by_holder'] += amount
        log('Repaid %d; debt now %d' % (amount, g['lien']['debt']))
        if g['lien']['debt'] == 0:
            g['lien']['closed_day'] = day
            g['history'] = g.get('history', []) + [g['lien']]
            g['lien'] = None
            log('Lien on %s released' % gid)

    elif action == 'cancel':
        need(actor == g['issuer'], 'Only the issuer can cancel')
        need(g['cancellable'], 'This grant is not cancellable')
        need(g['cancelled_at'] is None, 'Already cancelled')
        g['cancelled_at'] = day
        st = g.get('stake')
        if st:                                   # cancellation ends the stake; earned rewards still go to the holder
            if st['unpaid']:
                s['wallets'][g['holder']] += st['unpaid']
                s['supply'] += st['unpaid']
                st['paid'] += st['unpaid']
                st['unpaid'] = 0
            st['closed_day'] = day
            g['history'] = g.get('history', []) + [st]
            g['stake'] = None
        give_back = g['escrow'] - claimable(g, day)
        g['escrow'] -= give_back
        s['wallets'][g['issuer']] += give_back
        set_shortfall(s, gid, day)
        log('Issuer cancelled %s; %d unvested returned' % (gid, give_back))
    else:
        raise Rejected('Unknown action')

    assert_invariants(s)
    return s
