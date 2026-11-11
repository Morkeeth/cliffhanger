"""Loopback-only demo server for the NEAR vesting-loan POC. python3 server.py [--port 8931]"""
import json, os, sys, tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import intents
import loan

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, 'state.json')
PAGE = os.path.join(HERE, 'index.html')


def load():
    if os.path.exists(STATE):
        with open(STATE) as f:
            return json.load(f)
    return loan.initial()


def save(s):
    fd, tmp = tempfile.mkstemp(dir=HERE)
    with os.fdopen(fd, 'w') as f:
        json.dump(s, f, indent=1)
    os.replace(tmp, STATE)


def view(s, account='alice'):
    return {'day': s['day'], 'label': s['labels']['note'], 'wallets': s['wallets'],
            'grants': loan.options(s, account), 'transfers': s['transfers'][-6:],
            'events': s['events'][-12:],
            'aurora': intents.aurora_cards(),
            'liens': {k: g['lien'] for k, g in s['grants'].items()}}


DESK = os.path.join(HERE, 'dashboards.src.html')
PRICE_BASIS = 2.0   # demo price for CLIFF rows; the page labels it demo
DEMO_FUND = {'id': 't1', 'title': 'Ecosystem fund allocation', 'from': 'demo', 'token': 'CLIFF',
             'pos': {'locked': 2_000_000, 'price_now': PRICE_BASIS, 'months_left': 24, 'cliff_months_left': 6, 'cancellable': False, 'token': 'CLIFF'}}


def desk_data(s):
    """Every open grant in the demo contract state, plus one labelled demo fund, run through the planner."""
    import math, planner
    out = []
    for gid, g in s['grants'].items():
        left = loan.unvested(g, s['day'])
        days_left = g['start'] + g['duration'] - s['day']
        if left <= 0 or days_left <= 0:
            continue
        pos = {'locked': left, 'vested': loan.vested(g, s['day']), 'price_now': PRICE_BASIS,
               'months_left': max(1, math.ceil(days_left / 30.42)),
               'cliff_months_left': max(0, math.ceil((g['start'] + g['cliff'] - s['day']) / 30.42)),
               'cancellable': g['cancellable'], 'token': g.get('token', 'CLIFF')}
        committed = 'borrowed' if g.get('lien') else 'sold a slice' if g.get('sale') else 'staked' if g.get('stake') else None
        out.append({'id': gid, 'title': g['title'].replace(' (cancellable)', ''), 'from': 'engine',
                    'token': pos['token'], 'committed': committed, 'pos': pos, 'plan': planner.plan(pos)})
    out.append(dict(DEMO_FUND, committed=None, plan=planner.plan(DEMO_FUND['pos'])))
    for g in intents.AURORA_GRANTS:
        if g['unvested'] <= 0 or g['months_left'] <= 0:
            continue
        pos = {'locked': g['unvested'], 'vested': g['vested'], 'price_now': intents.AURORA_DEMO_PRICE,
               'months_left': g['months_left'], 'cliff_months_left': g['cliff_months_left'],
               'cancellable': g['cancellable'], 'token': 'AURORA'}
        out.append({'id': g['id'], 'title': g['title'], 'from': 'demo', 'token': 'AURORA',
                    'committed': None, 'pos': pos, 'plan': planner.plan(pos)})
    return out


def intent_plan(s, q):
    """Size a demo choice. The browser posts the returned dry requests. This process does not."""
    grant, choice = q['grant'], q['choice']
    card = next((c for c in intents.aurora_cards() if c['id'] == grant), None)
    if card is None:
        held = [o for o in loan.options(s, 'alice') if o['id'] == grant]
        if not held:
            raise ValueError('Unknown grant')
        card = held[0]
    amount = int(q['amount']) if q.get('amount') else None
    slice_tokens = int(q['slice']) if q.get('slice') else None
    return intents.plan_position(
        intents.position_from_card(card), choice,
        price=q.get('price'), amount=amount, slice_tokens=slice_tokens,
        entered_discount=q.get('discount'))


class H(BaseHTTPRequestHandler):
    def _send(self, code, body, ctype='application/json'):
        b = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        if self.path in ('/', '/index.html'):
            with open(PAGE, 'rb') as f:
                return self._send(200, f.read(), 'text/html; charset=utf-8')
        if self.path in ('/desk', '/desk/'):
            with open(DESK) as f:
                page = f.read().replace('__DATA__', json.dumps(desk_data(load())))
            return self._send(200, page.encode(), 'text/html; charset=utf-8')
        if self.path == '/api/state':
            return self._send(200, view(load()))
        if self.path.startswith('/api/preview'):
            from urllib.parse import urlparse, parse_qs
            q = parse_qs(urlparse(self.path).query)
            p = loan.preview_borrow(load(), q['grant'][0], int(q['amount'][0]))
            return self._send(200, p)
        if self.path.startswith('/api/intent-plan'):
            from urllib.parse import urlparse, parse_qs
            q = {k: v[0] for k, v in parse_qs(urlparse(self.path).query).items()}
            try:
                return self._send(200, intent_plan(load(), q))
            except (KeyError, ValueError) as e:
                return self._send(400, {'error': str(e)})
        if self.path.startswith('/api/plan'):
            from urllib.parse import urlparse, parse_qs
            import planner
            q = {k: v[0] for k, v in parse_qs(urlparse(self.path).query).items()}
            try:
                pos = {'locked': float(q['locked']), 'price_now': float(q['price']), 'months_left': int(q['months']),
                       'cliff_months_left': int(q.get('cliff', 0)), 'cancellable': q.get('cancellable') == '1'}
                entered = {k: float(q[k]) for k in ('sale_discount', 'stake_yield', 'loan_apr', 'slice') if k in q}
                return self._send(200, planner.plan(pos, entered))
            except (KeyError, ValueError) as e:
                return self._send(400, {'error': str(e)})
        self._send(404, {'error': 'not found'})

    def do_POST(self):
        n = int(self.headers.get('Content-Length', 0))
        try:
            data = json.loads(self.rfile.read(n) or b'{}')
        except ValueError:
            return self._send(400, {'error': 'bad json'})
        if self.path == '/api/reset':
            s = loan.initial(); save(s); return self._send(200, view(s))
        if not self.path.startswith('/api/act/'):
            return self._send(404, {'error': 'not found'})
        try:
            s = loan.apply(load(), self.path.rsplit('/', 1)[1], data)
        except loan.Rejected as e:
            return self._send(409, {'error': str(e)})
        save(s)
        self._send(200, view(s))

    def log_message(self, *a):
        pass


if __name__ == '__main__':
    port = int(sys.argv[sys.argv.index('--port') + 1]) if '--port' in sys.argv else 8931
    print('NEAR vesting-loan POC on http://127.0.0.1:%d (demo state only)' % port)
    ThreadingHTTPServer(('127.0.0.1', port), H).serve_forever()
