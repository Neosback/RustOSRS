"""Target-free policy inference; the host evaluates/replays actions separately."""
import argparse
import json
import random
import subprocess
from pathlib import Path
from client import LabClient
from splits import split_for
from .codec import ACTION, SCHEMAS, TOOLS, TOOL, allowed, decode
from .context import encode_context
from .data import digest, emit, write
from .fit import resolve_bundle
from .geometry import signature,similarity
from .baseline import generate as procedural


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
    baseline=data.get('baseline')
    if baseline and (digest(root/baseline['kit'])!=baseline['kit_sha256'] or digest(args.binary)!=baseline['binary_sha256']):
        raise ValueError('The frozen procedural kit or Lab binary changed; prepare a new dataset for this comparison')
    candidate_budget=baseline['candidates'] if baseline else 8
    references=[]
    for item in data['tasks']:
        if item['split']=='train':
            references.append((item['id'],signature(json.loads((root/'tasks'/f"{item['id']}.task.json").read_text())['target'])))
    results=[]
    blind=[]
    method_key={}
    for item in cases[:args.episodes]:
        if args.cancel_file and Path(args.cancel_file).exists():
            break
        for kind,suffix in [('build',''),('complete','.complete'),('repair','.repair')]:
            task=root/'tasks'/f"{item['id']}{suffix}.task.json"
            if not task.exists():
                continue
            request=argparse.Namespace(bundle=args.bundle,task=str(task),goal=str(root/'tasks'/f"{item['id']}.goal.json"),
                                       binary=args.binary,steps=args.steps,seed=7+len(results)*candidate_budget,temperature=0.6,cancel_file=args.cancel_file)
            policy=[]
            comparison=[]
            failures=[]
            for attempt in range(candidate_budget):
                if args.cancel_file and Path(args.cancel_file).exists(): break
                request.seed=7+len(results)*candidate_budget+attempt
                try:
                    candidate=run(model,bundle,request)
                    policy.append(candidate)
                    write(out/f"{item['id']}-{kind}-{attempt}.actions.json",candidate['actions'])
                except (ValueError,RuntimeError) as error:
                    failures.append({'method':'policy','seed':request.seed,'reason':str(error)})
                if baseline:
                    try: comparison.append(procedural(request,root/baseline['kit'],kind,request.seed))
                    except (ValueError,RuntimeError,subprocess.CalledProcessError) as error:
                        failures.append({'method':'procedural','seed':request.seed,'reason':str(error)})
            # Ranking uses validity and circulation only. The host's target score never selects outputs.
            rank=lambda r:(r['verdict']['usable'],r['verdict']['rooms_reachable'],-r['verdict']['blocked_doorways'])
            chosen=max(policy,key=rank) if policy else None
            selected_baseline=max(comparison,key=rank) if comparison else None
            near=None
            if chosen and references:
                near=max(({'source':ident,'object_jaccard':similarity(signature(chosen['snapshot']),sig)} for ident,sig in references),key=lambda r:r['object_jaccard'])
            diversity=[1-similarity(signature(a['snapshot']),signature(b['snapshot'])) for i,a in enumerate(policy) for b in policy[i+1:]]
            record={'task':item['id'],'group':item['group'],'kind':kind,'candidate_budget':candidate_budget,
                    'score':chosen['target_score'] if chosen else None,'exact':chosen['target_exact'] if chosen else False,
                    'usable':bool(chosen and chosen['verdict']['usable']),'valid_candidates':sum(r['verdict']['usable'] for r in policy),
                    'steps':len(chosen['actions']) if chosen else 0,'replay_verified':bool(chosen and chosen['replay_verified']),
                    'nearest_training_example':near,'mean_pairwise_object_distance':sum(diversity)/len(diversity) if diversity else None,
                    'procedural_usable':bool(selected_baseline and selected_baseline['verdict']['usable']) if baseline else None,
                    'procedural_score':selected_baseline['target_score'] if selected_baseline else None,'failures':failures}
            results.append(record)
            # Blind packets contain snapshots and requirements; the method key is a separate file.
            pair=[('policy',chosen),('procedural',selected_baseline)]
            random.Random(request.seed).shuffle(pair)
            for label,candidate in pair:
                if candidate:
                    identifier=f'comparison-{len(blind):04d}'
                    blind.append({'id':identifier,'task':item['id'],'kind':kind,'goal':json.loads(Path(request.goal).read_text()),'snapshot':candidate['snapshot']})
                    method_key[identifier]=label
            emit(stage='evaluate',episodes=len(results),message=f"Held-out {kind}: {item['id']}; equal candidate budgets")
    count=len(results)
    measured=[r['score'] for r in results if r['score'] is not None]
    report={'format':'rustosrs-policy-evaluation','version':1,'status':'experimental','split':args.split,
            'episodes':count,'groups':len({r['group'] for r in results}), 'policy_target_visible':False,
            'exact_rate':sum(r['exact'] for r in results)/count if count else None,
            'usable_rate':sum(r['usable'] for r in results)/count if count else None,
            'mean_target_score':sum(measured)/len(measured) if measured else None,
            'baseline_comparison':{'candidate_budget':candidate_budget,'procedural_usable_rate':sum(bool(r['procedural_usable']) for r in results)/count if count else None,'frozen':baseline} if baseline else None,
            'insufficient_evaluation_data':len({r['group'] for r in results})<10,
            'human_preference':None,'recommended':False,'results':results}
    write(out/'evaluation.json',report)
    write(out/'blind-review.json',blind)
    write(out/'blind-method-key.json',method_key)
    emit(stage='ready',report=str(out/'evaluation.json'),message='Held-out comparison saved; blind human review is still unmeasured')
