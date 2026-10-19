"""Frozen procedural comparison using the canonical Rust generator and the same host checks."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
from client import LabClient


def category(kind):
    return 0 if kind<=3 else 1 if kind<=8 else 2 if kind==22 else 3


def generate(args,kit,kind,seed):
    task=json.loads(Path(args.task).read_text())
    goal=json.loads(Path(args.goal).read_text())
    floors=goal.get('footprint') or []
    if not floors:
        raise ValueError('The procedural baseline requires a requested footprint')
    semantics={'rooms':goal.get('rooms') or [],'portals':goal.get('portals') or [],'anchor':None,'locked_parts':[]}
    brief={'kind':'house','width':task['width'],'height':task['height'],'floors':1,'seed':seed,'variation':0,'rooms':None,
           'sketch':{'origin':{'x':2,'y':2},'floors':floors,'semantics':semantics}}
    with tempfile.TemporaryDirectory(prefix='rustosrs-baseline-') as directory:
        path=Path(directory)
        (path/'brief.json').write_text(json.dumps(brief))
        result=subprocess.run([str(args.binary),'generate',str(kit),str(path/'brief.json')],capture_output=True,text=True,check=True)
        generated=json.loads(result.stdout)['snapshot']
        for channel,vertex in [('tiles',False),('heights',True),('objects',False)]:
            translated=[]
            for value in generated[channel]:
                value={**value,'x':value['x']-2,'y':value['y']-2}
                if not (0<=value['x']<task['width']+vertex and 0<=value['y']<task['height']+vertex):
                    if channel=='objects' or channel=='tiles' and (value.get('underlay') or value.get('overlay') or value.get('flags')):
                        raise ValueError('Procedural candidate exceeds the requested bounds')
                    continue
                translated.append(value)
            generated[channel]=translated
        initial=task['initial']
        if kind!='build':
            occupied={(o['x'],o['y'],o['plane'],category(o['kind'])) for o in initial['objects']}
            generated['objects']=[o for o in generated['objects'] if (o['x'],o['y'],o['plane'],category(o['kind'])) not in occupied]+initial['objects']
            painted={(t['x'],t['y'],t['plane']):t for t in generated['tiles']}
            painted.update({(t['x'],t['y'],t['plane']):t for t in initial['tiles'] if t.get('underlay') or t.get('overlay') or t.get('flags')})
            generated['tiles']=list(painted.values())
            generated['heights']=initial['heights']
        for lock in task.get('locks',[]):
            if lock not in generated['objects']:
                raise ValueError('Procedural candidate would change an author lock')
        # Baseline assets are infrastructure input, with source-stamped dimensions from the kit.
        source=json.loads(Path(kit).read_text())
        sizes={}
        for key in ['exterior_wall','interior_wall','door','window','corner']:
            if source.get(key): sizes[(source[key]['id'],source[key]['kind'])]=[1,1]
        palette={(p['id'],p['kind']):p for p in task['palette']}
        for o in generated['objects']:
            if (o['id'],o['kind']) not in palette:
                # Starter kit has one-tile walls and roof pieces; generic furniture is disallowed.
                if o['kind'] in [10,11]: raise ValueError('Baseline asset dimensions are unavailable')
                palette[(o['id'],o['kind'])]={'id':o['id'],'kind':o['kind'],'size':[1,1]}
        assessed=copy.deepcopy(task)
        assessed['initial']=generated
        assessed['palette']=list(palette.values())
        for key in ['underlay','overlay']:
            values=[t[key]['id'] if key=='overlay' else t[key] for t in generated['tiles'] if t.get(key)]
            assessed['overlays' if key=='overlay' else 'underlays']=sorted(set(assessed['overlays' if key=='overlay' else 'underlays']+values))
        names=dict(task.get('object_names',{}))
        for key,name in [('exterior_wall','wall'),('interior_wall','wall'),('door','door'),('window','window'),('corner','wall')]:
            if source.get(key): names[str(source[key]['id'])]=name
        assessed['object_names']=names
        (path/'assessment.json').write_text(json.dumps(assessed))
        with LabClient(args.binary) as client:
            observed=client.reset(path/'assessment.json',visible=False)
            verdict=client.call({'cmd':'inspect'})['verdict']
        return {'snapshot':generated,'verdict':verdict,'target_score':observed['score'],'target_exact':observed['breakdown']['exact']}
