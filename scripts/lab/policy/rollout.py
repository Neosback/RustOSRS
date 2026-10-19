"""Target-free policy inference; the host evaluates/replays actions separately."""
import argparse
import json
from pathlib import Path
from client import LabClient
from splits import split_for
from .codec import ACTION, SCHEMAS, TOOLS, TOOL, allowed, decode
from .context import encode_context
from .data import digest, emit, write
from .fit import resolve_bundle


def load_model(bundle):
    import mlx.core as mx
    from .model import ToolPolicy
    path,manifest=resolve_bundle(bundle)
    model=ToolPolicy(**manifest['config'])
    model.load_weights(str(path/'weights.safetensors'))
    model.eval()
    mx.eval(model.parameters())
    return model,manifest


def propose(model,observation,goal,context_limit,temperature):
    import mlx.core as mx
    context=encode_context(observation,goal,context_limit)+[ACTION]
    prefix=[]
    while not prefix or len(prefix)<=len(SCHEMAS[TOOLS[prefix[0]-TOOL]]):
        choices=allowed(prefix,observation['legal'],observation['current'])
        if not choices:
            return None
        tokens=mx.array(context+prefix,dtype=mx.int32)[None,:]
        scores=model(tokens)[0,-1,mx.array(choices)]
        index=int((mx.argmax(scores) if temperature<=0 else mx.random.categorical(scores/temperature)).item())
        prefix.append(choices[index])
    return decode(prefix,observation['legal'])


def run(model,manifest,args):
    import mlx.core as mx
    mx.random.seed(args.seed)
    goal=json.loads(Path(args.goal).read_text())
    actions=[]
    rejected=0
    seen=set()
    with LabClient(args.binary) as client:
        # Only the Rust host opens this task/target. The model receives public observations.
        client.reset(args.task,seed=args.seed,visible=False)
        for step in range(args.steps):
            if getattr(args,'cancel_file',None) and Path(args.cancel_file).exists():
                break
            observation=client.call({'cmd':'observe'})['observation']
            if 'target' in observation:
                raise ValueError('Host returned a private target to the policy')
            if not observation['legal']['remaining']:
                break
            chosen=None
            finish=False
            for _ in range(8):
                action=propose(model,observation,goal,manifest['config']['context'],args.temperature)
                if action is None:
                    finish=True
                    break
                key=json.dumps(action,sort_keys=True)
                if key in seen:
                    rejected+=1
                    continue
                if client.call({'cmd':'validate','actions':[action]})['valid'][0]:
                    chosen=action
                    break
                rejected+=1
            if finish or chosen is None:
                break
            client.step(chosen,observe=False)
            actions.append(chosen)
            seen.add(json.dumps(chosen,sort_keys=True))
            if step%8==0:
                emit(stage='rollout',step=step+1,rejected=rejected,message='Predicting and validating tool actions')
        verdict=client.call({'cmd':'inspect'})['verdict']
        final=client.call({'cmd':'observe'})
        snapshot=final['observation']['current']
        replay=client.call({'cmd':'replay','actions':actions})
        if replay['observation']['current']!=snapshot:
            raise ValueError('Policy action replay changed the output')
        return {'learned_policy':True,'policy_target_visible':False,'experimental':True,'seed':args.seed,
                'actions':actions,'rejected_proposals':rejected,'verdict':verdict,
                'target_score':final['score'],'target_exact':final['breakdown']['exact'],
                'snapshot':snapshot,'replay_verified':True}


def rollout(args):
    model,manifest=load_model(args.bundle)
    result=run(model,manifest,args)
    out=Path(args.out)
    out.parent.mkdir(parents=True,exist_ok=True)
    write(out,result['actions'])
    write(str(out)+'.metrics.json',{k:v for k,v in result.items() if k!='actions'})
    emit(stage='ready',actions=str(out),steps=len(result['actions']),rejected=result['rejected_proposals'],message='Experimental policy rollout replay verified; review before keeping')
    return result


def evaluate(args):
    model,bundle=load_model(args.bundle)
    root=Path(args.dataset)
    data=json.loads((root/'manifest.json').read_text())
    if bundle['dataset_sha256']!=digest(root/'manifest.json'):
        raise ValueError('Evaluation requires the frozen dataset used by the bundle')
    training={g for g,s in bundle['groups'].items() if s=='train'}
    cases=[t for t in data['tasks'] if t['split']==args.split]
    if any(t['group'] in training or split_for(t['group'])!=args.split for t in cases):
        raise ValueError('Held-out evaluation has a training-group overlap')
    out=Path(args.out)
    out.mkdir(parents=True,exist_ok=True)
    results=[]
    for item in cases[:args.episodes]:
        if args.cancel_file and Path(args.cancel_file).exists():
            break
        for kind,suffix in [('build',''),('complete','.complete'),('repair','.repair')]:
            task=root/'tasks'/f"{item['id']}{suffix}.task.json"
            if not task.exists():
                continue
            request=argparse.Namespace(bundle=args.bundle,task=str(task),goal=str(root/'tasks'/f"{item['id']}.goal.json"),
                                       binary=args.binary,steps=args.steps,seed=7+len(results),temperature=0.6)
            result=run(model,bundle,request)
            write(out/f"{item['id']}-{kind}.actions.json",result['actions'])
            results.append({'task':item['id'],'group':item['group'],'kind':kind,'score':result['target_score'],
                            'exact':result['target_exact'],'usable':result['verdict']['usable'],
                            'steps':len(result['actions']),'replay_verified':result['replay_verified']})
            emit(stage='evaluate',episodes=len(results),message=f"Held-out {kind}: {item['id']}")
    count=len(results)
    report={'format':'rustosrs-policy-evaluation','version':1,'status':'experimental','split':args.split,
            'episodes':count,'groups':len({r['group'] for r in results}), 'policy_target_visible':False,
            'exact_rate':sum(r['exact'] for r in results)/count if count else None,
            'usable_rate':sum(r['usable'] for r in results)/count if count else None,
            'mean_target_score':sum(r['score'] for r in results)/count if count else None,
            'baseline_comparison':None,'human_preference':None,'recommended':False,'results':results}
    write(out/'evaluation.json',report)
    emit(stage='ready',report=str(out/'evaluation.json'),message='Held-out evaluation saved; baseline and human review remain separate')
