"""Target-free scene encoding. Oversize examples fail explicitly rather than dropping geometry."""
import hashlib
import struct
from collections import defaultdict
from .codec import BOS, GOAL, PAL, OBJECT, TILE, HEIGHT, ROOM, ASSET, asset_index


def rectangles(cells):
    remaining = set(cells)
    while remaining:
        x,y = min(remaining,key=lambda c:(c[1],c[0]))
        w=1
        while (x+w,y) in remaining:
            w+=1
        h=1
        while all((x+i,y+h) in remaining for i in range(w)):
            h+=1
        remaining.difference_update((x+i,y+j) for i in range(w) for j in range(h))
        yield x,y,w,h


def goal_for(prefab):
    building = prefab.get('building') or {}
    return {'width':prefab['width'],'height':prefab['height'],'planes':prefab['planes'],
            'footprint':building.get('floors') or [{'x':t['x'],'y':t['y'],'plane':t['plane']} for t in prefab['tiles']],
            'rooms':building.get('semantics',{}).get('rooms',[]),
            'locks':[], 'style':next(iter(prefab.get('tags',[])),'default')}


def encode_context(observation, goal, limit=1024):
    if 'target' in observation:
        raise ValueError('A policy observation must not contain a target')
    legal,current = observation['legal'],observation['current']
    if max(legal['width'],legal['height']) > 24 or len(legal['palette']) > 512 or any(len(legal.get(k,[])) > 254 for k in ['overlays','underlays']):
        raise ValueError('Policy v1 supports sites up to 24x24, 512 assets, 254 floor materials')
    tokens=[BOS,GOAL,legal['width'],legal['height'],legal['planes'],min(legal['remaining'],255)]
    floor=defaultdict(set)
    for c in goal.get('footprint',[]):
        floor[c.get('plane',0)].add((c['x'],c['y']))
    for plane,cells in sorted(floor.items()):
        for x,y,w,h in rectangles(cells):
            tokens += [GOAL,plane,x,y,w,h]
    for room in goal.get('rooms') or []:
        cells=room.get('cells',[])
        if not cells:
            continue
        role=hashlib.sha256(room.get('role','general').encode()).digest()[0]
        xs,ys=[c['x'] for c in cells],[c['y'] for c in cells]
        tokens += [ROOM,role,min(xs),min(ys),max(xs)-min(xs)+1,max(ys)-min(ys)+1,cells[0].get('plane',0)]
    for i,p in enumerate(legal['palette'],1):
        name=legal.get('object_names',{}).get(str(p['id']),'')
        role=hashlib.sha256(name.encode()).digest()[0] if name else 0
        tokens += [PAL,ASSET+i,p['kind'],*p['size'],role]
    for o in sorted(current['objects'],key=lambda o:(o['plane'],o['y'],o['x'],o['kind'],o['id'])):
        tokens += [OBJECT,ASSET+asset_index(legal,o),o['x'],o['y'],o['plane'],o['orientation']]
    for o in legal.get('locks',[]):
        tokens += [809,ASSET+asset_index(legal,o),o['x'],o['y'],o['plane'],o['orientation']]
    groups=defaultdict(set)
    for t in current['tiles']:
        overlay=t.get('overlay') or {}
        u=legal.get('underlays',[])
        v=legal.get('overlays',[])
        key=(t['plane'],u.index(t['underlay'])+1 if t.get('underlay') in u else 0,
             v.index(overlay.get('id'))+1 if overlay.get('id') in v else 0,overlay.get('shape',0),overlay.get('rotation',0),t.get('flags',0))
        groups[key].add((t['x'],t['y']))
    for (p,u,v,s,r,f),cells in sorted(groups.items()):
        for x,y,w,h in rectangles(cells):
            tokens += [TILE,x,y,w,h,p,u,v,s,r,f]
    groups=defaultdict(set)
    for h in current['heights']:
        groups[(h['plane'],h['value'])].add((h['x'],h['y']))
    for (p,value),cells in sorted(groups.items()):
        for x,y,w,h in rectangles(cells):
            tokens += [HEIGHT,x,y,w,h,p,*struct.pack('<i',value)]
    if len(tokens)+20 > limit:
        raise ValueError(f'Scene needs {len(tokens)+20} tokens; policy context limit is {limit}')
    return tokens
