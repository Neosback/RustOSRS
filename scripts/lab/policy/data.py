"""Verified teacher demonstrations, partial completion and corruption repair; frozen source groups."""
import copy
import hashlib
import json
from pathlib import Path
import random
import subprocess
from collections import Counter
from client import LabClient
from splits import group_for, split_for
from .codec import ACTION, encode
from .context import encode_context, goal_for
from .geometry import identity


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write(path,value):
    Path(path).write_text(json.dumps(value,sort_keys=True,indent=2)+'\n')


def emit(**value):
    print(json.dumps(value),flush=True)


def corpus_prefab(record,objects):
    if not record.get('coverage_complete') or record.get('review'):
        raise ValueError('Incomplete or ambiguous source coverage')
    floors=record['floor_tiles']
    base=min(c['plane'] for c in floors)
    if len({c['plane'] for c in floors}) != 1:
        raise ValueError('Policy v1 trains single-floor buildings; upper roofs are retained')
    members=record['objects']
    sizes={str(o['object']):objects[str(o['object'])]['size'] for o in members}
    points=[tuple(c['tile']) for c in floors]+[tuple(o['tile']) for o in members]
    x0,y0=min(t[0] for t in points),min(t[1] for t in points)
    def cell(tile,plane):
        return {'x':tile[0]-x0,'y':tile[1]-y0,'plane':plane-base}
    width=max([t[0]-x0+1 for t in points]+[o['tile'][0]-x0+sizes[str(o['object'])][o['orientation']&1] for o in members])
    height=max([t[1]-y0+1 for t in points]+[o['tile'][1]-y0+sizes[str(o['object'])][1-(o['orientation']&1)] for o in members])
    if max(width,height)>24:
        raise ValueError('Building exceeds the initial 24x24 policy limit')
    owned=[cell(c['tile'],c['plane']) for c in floors]
    terrain={(t['plane'],*t['tile']):t for t in record['terrain']}
    tiles=[]
    for f in floors:
        t=terrain[(f['plane'],*f['tile'])]
        tiles.append({**cell(f['tile'],f['plane']),'underlay':t['underlay'] or None,
                      'overlay':{'id':t['overlay'],'shape':t['shape'],'rotation':t['rotation']} if t['overlay'] else None,'flags':t['flags']})
    heights=[{**cell(v['vertex'],v['plane']),'value':v['height']} for v in record['height_vertices']
             if base<=v['plane'] and 0<=v['vertex'][0]-x0<=width and 0<=v['vertex'][1]-y0<=height]
    rooms=[{'id':i,'role':r['tag'],'cells':[cell(t,r['plane']) for t in r['tiles']], 'human':False} for i,r in enumerate(record['rooms']) if r['plane']==base]
    p={'format':'rustosrs-prefab','version':3,'name':record['id'],'tags':['cache-corpus'],'notes':'Weak census labels; geometry is source data',
       'source':{'from':'captured','tile_x':record['origin'][0]+x0,'tile_y':record['origin'][1]+y0,'plane':base},
       'width':width,'height':height,'planes':max([o['plane'] for o in members]+[base])-base+1,'tiles':tiles,'heights':heights,
       'objects':[{**cell(o['tile'],o['plane']),'id':o['object'],'kind':o['kind'],'orientation':o['orientation']} for o in members],
       'building':{'floors':owned,'support':[{k:h[k] for k in ['x','y','plane']} for h in heights],
                   'source_datum':sorted(h['value'] for h in heights if h['plane']==0)[len([h for h in heights if h['plane']==0])//2],
                   'object_sizes':sizes,'provenance':json.dumps(record['source'],sort_keys=True),'relations':[],'labels':{},'reviewed':False,
                   'semantics':{'rooms':rooms,'portals':[],'anchor':None,'locked_parts':[]}}}
    # Source plane-relative height entries outside the used roof planes are not part of this task.
    p['heights']=[h for h in heights if h['plane']<p['planes']]
    p['building']['support']=[c for c in p['building']['support'] if c['plane']<p['planes']]
    return p


def sources(prefabs,corpus):
    if prefabs:
        for path in sorted(Path(prefabs).glob('*.prefab.json')):
            p=json.loads(path.read_text())
            if not (p.get('building') or {}).get('reviewed'):
                continue
            yield str(path),p,None
    if corpus:
        objects=json.loads((Path(corpus)/'objects.json').read_text())
        with (Path(corpus)/'buildings.jsonl').open() as handle:
            for line in handle:
                r=json.loads(line)
                try:
                    yield r['id'],corpus_prefab(r,objects),r.get('group')
                except (KeyError,ValueError,IndexError) as error:
                    yield r.get('id','unknown'),None,str(error)


def trajectory(client,path,goal,limit):
    from train import imitate
    observed=client.reset(path,visible=True)['observation']
    actions=list(imitate(client,observed))
    rows=[]
    for action in actions+[None]:
        observation=client.call({'cmd':'observe'})['observation']
        observation.pop('target',None)
        context=encode_context(observation,goal,limit)
        rows.append({'tokens':context+[ACTION]+encode(action,observation['legal']), 'action_start':len(context)+1})
        if action is not None:
            client.step(action,observe=False)
    # Re-execute from the immutable initial state, independently of the demonstration run.
    replay=client.call({'cmd':'replay','actions':actions})
    if not replay['breakdown']['exact']:
        raise ValueError('Teacher replay is not exact; excluded from training')
    return rows,actions,replay['observation']['current']


def prepare(args):
    out=Path(args.out)
    if (out/'manifest.json').exists():
        raise ValueError('Use a new dataset directory; prepared splits are immutable')
    out.mkdir(parents=True,exist_ok=True)
    tasks=out/'tasks'
    tasks.mkdir(exist_ok=True)
    handles={s:(out/f'{s}.jsonl').open('w') for s in ['train','validation','test']}
    counts=Counter()
    groups={}
    entries=[]
    palettes={}
    rejected=[]
    seen={}
    try:
        with LabClient(args.binary) as client:
            for source,p,source_group in sources(args.prefabs,args.corpus):
                if args.cancel_file and Path(args.cancel_file).exists():
                    break
                if p is None:
                    rejected.append({'source':source,'reason':source_group})
                    continue
                if args.max_buildings and len(entries)>=args.max_buildings:
                    break
                if not any(o['kind'] in (0,2) for o in p['objects']):
                    rejected.append({'source':source,'reason':'No building shell; blank scenes are not training buildings'})
                    continue
                group=source_group or group_for(p)
                split=split_for(group)
                # Exact geometry copies must not enter different source groups.
                geometry=identity(p)
                if geometry in seen:
                    rejected.append({'source':source,'reason':'duplicate geometry','family':seen[geometry]})
                    continue
                seen[geometry]=group
                ident=hashlib.sha256((source+geometry).encode()).hexdigest()[:16]
                prefab=tasks/f'{ident}.prefab.json'
                path=tasks/f'{ident}.task.json'
                write(prefab,p)
                try:
                    result=subprocess.run([str(args.binary),'task-copy',str(prefab)],capture_output=True,text=True,check=True)
                    task=json.loads(result.stdout)
                    names={}
                    if args.corpus:
                        infos=json.loads((Path(args.corpus)/'objects.json').read_text())
                        names={str(o['id']):infos.get(str(o['id']),{}).get('name','') for o in p['objects']}
                    task['object_names']=names
                    task['group']=group
                    task['budget']=max(4000,task['budget'])
                    write(path,task)
                    goal=goal_for(p)
                    write(tasks/f'{ident}.goal.json',goal)
                    rows,actions,_=trajectory(client,path,goal,args.context)
                    variants=[('build',rows)]
                    if actions:
                        client.reset(path,visible=False)
                        for action in actions[:len(actions)//2]:
                            client.step(action,observe=False)
                        partial=copy.deepcopy(task)
                        partial['initial']=client.call({'cmd':'observe'})['observation']['current']
                        part_path=tasks/f'{ident}.complete.task.json'
                        write(part_path,partial)
                        partial_rows,_,_=trajectory(client,part_path,goal,args.context)
                        variants.append(('complete',partial_rows))
                    repair=copy.deepcopy(task)
                    repair['initial']=copy.deepcopy(task['target'])
                    repair['initial']['objects']=[o for i,o in enumerate(repair['initial']['objects']) if i%5]
                    repair_path=tasks/f'{ident}.repair.task.json'
                    write(repair_path,repair)
                    repair_rows,_,_=trajectory(client,repair_path,goal,args.context)
                    variants.append(('repair',repair_rows))
                    groups[group]=split
                    for kind,rows in variants:
                        for row in rows:
                            row.update(group=group,task=ident,kind=kind,policy_target_visible=False)
                            handles[split].write(json.dumps(row,separators=(',',':'))+'\n')
                            counts[split]+=1
                    if split=='train':
                        style=(p.get('building') or {}).get('labels',{}).get('human:style') or group
                        bound=palettes.setdefault(style,{'palette':{},'overlays':set(),'underlays':set(),'object_names':{}})
                        for asset in task['palette']:
                            bound['palette'][(asset['id'],asset['kind'])]=asset
                        bound['overlays'].update(task['overlays'])
                        bound['underlays'].update(task['underlays'])
                        bound['object_names'].update(names)
                    entries.append({'id':ident,'group':group,'split':split,'source':source,'source_sha256':digest(prefab),
                                    'geometry_sha256':geometry,'teacher_exact':True,'label_source':'reviewed-prefab' if p['building']['reviewed'] else 'weak-cache-census'})
                    emit(stage='prepare',buildings=len(entries),examples=dict(counts),message=f'Verified {source}')
                except (ValueError,KeyError,RuntimeError,StopIteration,subprocess.CalledProcessError) as error:
                    rejected.append({'source':source,'reason':str(error)[:1200]})
    finally:
        for handle in handles.values():
            handle.close()
    palettes={name:{**v,'palette':list(v['palette'].values()),'overlays':sorted(v['overlays']),'underlays':sorted(v['underlays'])} for name,v in palettes.items()}
    baseline_source=Path(__file__).resolve().parents[3]/'crates/osrs-editor/src/app/prefab_runtime/kits/lumbridge-brick.kit.json'
    baseline=out/'procedural-baseline.kit.json'
    baseline.write_bytes(baseline_source.read_bytes())
    manifest={'baseline':{'kit':baseline.name,'kit_sha256':digest(baseline),'binary_sha256':digest(args.binary),'candidates':8},'style_palettes':palettes,'format':'rustosrs-policy-dataset','version':1,'context':args.context,'groups':groups,'examples':dict(counts),'tasks':entries,'rejected':rejected,
              'split_algorithm':'geography-v1/fnv1a-family-v1','sha256':{s:digest(out/f'{s}.jsonl') for s in handles},
              'teacher_target_visible':True,'policy_target_visible':False}
    write(out/'manifest.json',manifest)
    if not counts['train']:
        raise ValueError(f'No usable training examples; inspect {out / "manifest.json"}')
    emit(stage='ready',dataset=str(out),examples=dict(counts),rejected=len(rejected),message='Dataset frozen; teacher traces verified')
