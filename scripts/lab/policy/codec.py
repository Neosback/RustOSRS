"""Fixed, versioned tool grammar. Asset tokens are local palette pointers, never raw target IDs."""
import struct

ASSET = 256
TOOLS = ['paint_rect', 'height_stroke', 'place', 'remove', 'wall_line', 'room_rect', 'move', 'turn', 'hip_roof', 'finish']
TOOL = 769
BOS, GOAL, PAL, OBJECT, TILE, HEIGHT, ACTION, PAD, ROOM = range(800, 809)
VOCAB = 832
SCHEMAS = {
    'paint_rect': ['x','y','width','height','plane','overlay','underlay','shape','rotation','flags'],
    'height_stroke': ['x','y','width','height','plane','b0','b1','b2','b3'],
    'place': ['asset','x','y','plane','orientation'],
    'remove': ['asset','x','y','plane','orientation'],
    'wall_line': ['asset','x','y','plane','orientation','length','vertical'],
    'room_rect': ['asset','x','y','plane','orientation','width','height','corner'],
    'move': ['asset','x','y','plane','orientation','dx','dy'],
    'turn': ['asset','x','y','plane','orientation','turns'],
    'hip_roof': ['plane','x0','y0','x1','y1','slope','edge','ridge'],
    'finish': [],
}


def asset_index(legal, obj):
    return next(i+1 for i,p in enumerate(legal['palette']) if (p['id'],p['kind']) == (obj['id'],obj['kind']))


def encode(action, legal):
    if action is None:
        return [TOOL + TOOLS.index('finish')]
    tool = action['tool']
    values = dict(action)
    if 'object' in action:
        values.update(action['object'])
        values['asset'] = ASSET + asset_index(legal, action['object'])
    if tool == 'paint_rect':
        for field, palette in [('overlay','overlays'),('underlay','underlays')]:
            value = action.get(field)
            values[field] = 0 if value is None else legal[palette].index(value)+1
        values['flags'] = 256 if action.get('flags') is None else action['flags']
    if tool == 'height_stroke':
        values.update(zip(['b0','b1','b2','b3'],struct.pack('<i',action['value'])))
    if tool == 'room_rect':
        values['corner'] = ASSET + (0 if action.get('corner_id') is None else next(i+1 for i,p in enumerate(legal['palette']) if p['id'] == action['corner_id']))
    if tool == 'hip_roof':
        for field, name in [('slope','slope_id'),('edge','edge_id'),('ridge','ridge_id')]:
            value = action.get(name)
            values[field] = ASSET + (0 if value is None else next(i+1 for i,p in enumerate(legal['palette']) if p['id'] == value))
    for field in ['dx','dy']:
        if field in values:
            values[field] += 128
    values['vertical'] = int(action.get('vertical',False))
    tokens = [TOOL + TOOLS.index(tool)] + [values[f] for f in SCHEMAS[tool]]
    if any(not isinstance(t,int) or t < 0 or t >= VOCAB for t in tokens):
        raise ValueError('Action exceeds policy vocabulary')
    return tokens


def decode(tokens, legal):
    tool = TOOLS[tokens[0]-TOOL]
    if tool == 'finish':
        return None
    fields = SCHEMAS[tool]
    if len(tokens) != len(fields)+1:
        raise ValueError('Incomplete action')
    values = dict(zip(fields,tokens[1:]))
    action = {'tool':tool}
    if 'asset' in values:
        entry = legal['palette'][values.pop('asset')-ASSET-1]
        action['object'] = {'id':entry['id'],'kind':entry['kind'],**{f:values.pop(f) for f in ['x','y','plane','orientation']}}
    if tool == 'paint_rect':
        for field,palette in [('overlay','overlays'),('underlay','underlays')]:
            index = values[field]
            values[field] = None if index == 0 else legal[palette][index-1]
        if values['flags'] == 256:
            values['flags'] = None
    if tool == 'height_stroke':
        values['value'] = struct.unpack('<i',bytes(values.pop(f'b{i}') for i in range(4)))[0]
    if tool == 'room_rect':
        index = values.pop('corner')-ASSET
        values['corner_id'] = legal['palette'][index-1]['id'] if index else None
    if tool == 'hip_roof':
        for field,name in [('slope','slope_id'),('edge','edge_id'),('ridge','ridge_id')]:
            index = values.pop(field)-ASSET
            values[name] = legal['palette'][index-1]['id'] if index else None
    for field in ['dx','dy']:
        if field in values:
            values[field] -= 128
    if 'vertical' in values:
        values['vertical'] = bool(values['vertical'])
    action.update(values)
    return action


def allowed(prefix, legal, current):
    """Syntactic/palette/bounds masks; Rust independently checks the final action and scene slots."""
    if not prefix:
        tools = ['finish']
        if legal.get('palette'):
            tools += ['place','wall_line','room_rect','hip_roof']
        if current.get('objects'):
            tools += ['remove','move','turn']
        if legal.get('underlays') or legal.get('overlays'):
            tools += ['paint_rect']
        tools += ['height_stroke']
        return [TOOL+TOOLS.index(t) for t in tools]
    tool = TOOLS[prefix[0]-TOOL]
    fields = SCHEMAS[tool]
    index = len(prefix)-1
    if index >= len(fields):
        return []
    field = fields[index]
    past = dict(zip(fields,prefix[1:]))
    w,h = legal['width'],legal['height']
    if field in ['asset','slope','edge','ridge','corner']:
        candidates = list(enumerate(legal['palette'],1))
        if tool in ['remove','move','turn']:
            present = {(o['id'],o['kind']) for o in current['objects']}
            candidates = [(i,p) for i,p in candidates if (p['id'],p['kind']) in present]
        if tool in ['wall_line','room_rect']:
            candidates = [(i,p) for i,p in candidates if p['kind'] == 0]
        result = [ASSET+i for i,_ in candidates]
        if field in ['edge','ridge','corner']:
            result.insert(0,ASSET)
        return result
    if tool in ['remove','move','turn'] and field in ['x','y','plane','orientation']:
        entry = legal['palette'][past['asset']-ASSET-1]
        matches = [o for o in current['objects'] if o not in legal.get('locks',[]) and (o['id'],o['kind']) == (entry['id'],entry['kind']) and all(o[k] == v for k,v in past.items() if k in ['x','y','plane','orientation'])]
        return sorted({o[field] for o in matches})
    ranges = {'x':w,'x0':w,'x1':w,'y':h,'y0':h,'y1':h,'plane':legal['planes'], 'orientation':4,'shape':12,'rotation':4,'vertical':2,'flags':257,
              'overlay':len(legal.get('overlays',[]))+1,'underlay':len(legal.get('underlays',[]))+1}
    if field in ranges:
        lo = past.get('x0',0) if field == 'x1' else past.get('y0',0) if field == 'y1' else 0
        return list(range(lo,ranges[field]))
    if field == 'width':
        return list(range(1,w-past.get('x',0)+1))
    if field == 'height':
        return list(range(1,h-past.get('y',0)+1))
    if field == 'length':
        return list(range(1,max(w,h)+1))
    if field == 'turns':
        return [1,2,3]
    if field.startswith('b'):
        heights={h['value'] for h in current.get('heights',[])} or {0}
        options=[struct.pack('<i',v) for v in heights]
        matching=[v for v in options if all(v[int(f[1])] == value for f,value in past.items() if f.startswith('b'))]
        return sorted({v[int(field[1])] for v in matching})
    return list(range(256))
