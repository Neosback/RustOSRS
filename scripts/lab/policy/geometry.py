"""Rotation-aware geometry identity and transparent nearest-source similarity."""
import copy
import hashlib
import json


def rotations(prefab):
    p=copy.deepcopy(prefab)
    for _ in range(4):
        yield p
        w=p['width']
        sizes=(p.get('building') or {}).get('object_sizes',{})
        for t in p.get('tiles',[]):
            t['x'],t['y']=t['y'],w-1-t['x']
            if t.get('overlay'):
                t['overlay']['rotation']=(t['overlay']['rotation']+1)%4
        for h in p.get('heights',[]):
            h['x'],h['y']=h['y'],w-h['x']
        for o in p.get('objects',[]):
            size=sizes.get(str(o['id']),[1,1])
            fw=size[o['orientation']%2] if o['kind'] in [10,11] else 1
            o['x'],o['y']=o['y'],w-fw-o['x']
            o['orientation']=(o['orientation']+1)%4
        p['width'],p['height']=p['height'],p['width']


def identity(prefab):
    variants=[]
    for p in rotations(prefab):
        # Ignore labels, ordering, source location, and absolute elevation.
        datum=min((h['value'] for h in p.get('heights',[])),default=0)
        body=[p['width'],p['height'],
              sorted((json.dumps(t,sort_keys=True) for t in p.get('tiles',[]))),
              sorted((json.dumps(o,sort_keys=True) for o in p.get('objects',[]))),
              sorted((h['x'],h['y'],h['plane'],h['value']-datum) for h in p.get('heights',[]))]
        variants.append(json.dumps(body,sort_keys=True))
    return hashlib.sha256(min(variants).encode()).hexdigest()


def signature(snapshot):
    return {(o['id'],o['kind'],o['plane'],o['x'],o['y'],o['orientation']) for o in snapshot.get('objects',[])}


def similarity(a,b):
    union=a|b
    return len(a&b)/len(union) if union else 0.0
