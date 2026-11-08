"""Cliffhanger treasury planner: what each choice does to one locked position under three price paths.

A pure function. Mirrors the engine in loan.py: sale and loan are token-denominated, a cancellable
grant cannot be sold or borrowed against, and stake rewards accrue on the escrow that is still locked.
Every assumption carries a label: 'sourced' (with its source), 'demo' or 'you entered it'.
"""

SOURCED, DEMO, USER = 'sourced', 'demo', 'you entered it'
OFFX = 'OFFX (Jonas Thiele) in The Block, 5 Apr 2026'
KEYROCK = 'Keyrock, From Locked to Liquidity: 16,000+ token unlocks'


def offx_discount(months):
    """Median discount to sell locked tokens early, by months until unlock (OFFX bands)."""
    if months <= 12:
        return 0.40
    if months <= 24:
        return 0.50
    if months >= 36:
        return 0.60
    return None  # 25 to 35 months: OFFX gives no band; the planner asks the user


def default_assumptions(position):
    m = position['months_left']
    d = offx_discount(m)
    a = {
        'unlock_drop': {'value': 0.25, 'label': SOURCED, 'source': KEYROCK + ': team unlocks averaged -25%'},
        'price_up': {'value': 0.25, 'label': DEMO, 'source': 'a symmetric demo path, not a forecast'},
        'stake_yield': {'value': 0.045, 'label': DEMO,
                        'source': 'about 4.5% as reported after NEAR cut max inflation to 2.5% (NEAR One, 21 Oct 2025); secondary reports'},
        'loan_ltv': {'value': 0.40, 'label': DEMO, 'source': 'Cliffhanger demo limit (loan.py LTV_FIXED)'},
        'loan_apr': {'value': 0.12, 'label': DEMO, 'source': 'Cliffhanger demo fee (loan.py APR), not a market price'},
        'slice': {'value': 0.25, 'label': DEMO, 'source': 'share of the locked tokens offered for sale'},
    }
    if d is not None:
        a['sale_discount'] = {'value': d, 'label': SOURCED, 'source': OFFX + ': median for %d months' % m}
    return a


def merge(defaults, entered):
    out = {k: dict(v) for k, v in defaults.items()}
    for k, v in (entered or {}).items():
        out[k] = {'value': v, 'label': USER, 'source': 'you entered it'}
    missing = [k for k in ('sale_discount',) if k not in out]
    if missing:
        raise ValueError('No sourced value for %s at this term; enter one' % ', '.join(missing))
    return out


def _escrow_months(position):
    """Locked tokens at the start of each remaining month."""
    q, m, cliff = position['locked'], position['months_left'], position.get('cliff_months_left', 0)
    # Same shape as the engine: vesting accrues from the start, the cliff releases what has accrued.
    return [q if i < cliff else q * (m - i) / m for i in range(m)]


def _money(x):
    return round(x, 2)


def _row(cash_now, kept_tokens, rewards_tokens, p_end, note):
    kept, rewards = _money(kept_tokens * p_end), _money(rewards_tokens * p_end)
    cash = _money(cash_now)
    return {'cash_now': cash, 'kept_value': kept, 'rewards_value': rewards,
            'total': _money(cash + kept + rewards), 'note': note}


def plan(position, entered=None):
    """position: locked (tokens), price_now, months_left, cliff_months_left, cancellable.
    entered: assumption overrides from the user. Returns scenarios, choices for both seats, worst case."""
    q, p0, m = position['locked'], position['price_now'], position['months_left']
    if q <= 0 or p0 <= 0 or m <= 0:
        raise ValueError('Locked tokens, price and months left must all be above zero')
    a = merge(default_assumptions(position), entered)
    v = {k: x['value'] for k, x in a.items()}
    paths = {'down': p0 * (1 - v['unlock_drop']), 'flat': p0, 'up': p0 * (1 + v['price_up'])}
    path_labels = {'down': a['unlock_drop']['label'], 'flat': DEMO, 'up': a['price_up']['label']}
    years = m / 12
    rewards = sum(e * v['stake_yield'] / 12 for e in _escrow_months(position))
    slice_t = q * v['slice']
    sale_price_t = slice_t * (1 - v['sale_discount'])
    advance_t = q * v['loan_ltv']
    fee_t = advance_t * v['loan_apr'] * years
    locked_out = position.get('cancellable', False)
    why = 'A cancellable grant can vanish, so it cannot back a sale or a loan' if locked_out else None

    holder, funder = {}, {}
    for name, p_end in paths.items():
        h = {'hold': _row(0, q, 0, p_end, 'wait for every unlock'),
             'stake': _row(0, q, rewards, p_end, 'rewards on the escrow still locked, assuming you claim each unlock as it arrives')}
        f = {}
        if not locked_out:
            h['sell'] = _row(sale_price_t * p0, q - slice_t, 0, p_end,
                             'buyer pays now at a %d%% discount and is paid first from unlocks' % round(v['sale_discount'] * 100))
            h['borrow'] = _row(advance_t * p0, q - advance_t - fee_t, 0, p_end,
                               'each unlock repays the lender first, fee included')
            f['buy_slice'] = {'paid_now': _money(sale_price_t * p0), 'received_value': _money(slice_t * p_end)}
            f['lend'] = {'paid_now': _money(advance_t * p0), 'received_value': _money((advance_t + fee_t) * p_end)}
            for x in f.values():
                x['gain'] = _money(x['received_value'] - x['paid_now'])
        for r in h.values():
            r['vs_hold'] = _money(r['total'] - h['hold']['total'])
        holder[name], funder[name] = h, f

    def worst(table, key):
        cells = [(s, c, row[key]) for s, chs in table.items() for c, row in chs.items()]
        s, c, val = min(cells, key=lambda x: x[2]) if cells else (None, None, None)
        return {'scenario': s, 'choice': c, key: val}

    cancel = None
    if locked_out:
        cancel = {'what': 'If the grant is cancelled, unvested tokens return to the issuer',
                  'stake': 'the holder keeps vested tokens and rewards earned so far'}
    else:
        cancel = {'what': 'This grant cannot be cancelled, so unlocks arrive as scheduled'}
    return {'assumptions': a, 'paths': {k: {'price': _money(p), 'label': path_labels[k]} for k, p in paths.items()},
            'holder': holder, 'funder': funder, 'unavailable': why, 'cancel': cancel,
            'worst_holder': worst(holder, 'total'), 'worst_funder': worst(funder, 'gain')}
