"""Bounded MLX fitting with immutable checkpoints, exact resume, and validation-only selection."""
import json
import math
from pathlib import Path
import random
import resource
import signal
import sys
import time
from .data import digest, emit, write


def atomic_reference(path,value):
    temporary=path.with_suffix('.tmp')
    write(temporary,value)
    temporary.replace(path)


def resolve_bundle(root,latest=False):
    root=Path(root)
    if (root/'bundle.json').exists():
        checkpoint=root
    else:
        ref=root/('latest.json' if latest or not (root/'best.json').exists() else 'best.json')
        checkpoint=root/json.loads(ref.read_text())['checkpoint']
    manifest=json.loads((checkpoint/'bundle.json').read_text())
    if manifest.get('format')!='rustosrs-tool-policy' or manifest.get('version')!=1:
        raise ValueError('Unsupported policy bundle')
    for name,expected in manifest['sha256'].items():
        if Path(name).name!=name or digest(checkpoint/name)!=expected:
            raise ValueError(f'Policy checkpoint hash mismatch: {name}')
    return checkpoint,manifest


def offsets(path):
    result=[]
    with Path(path).open('rb') as handle:
        while True:
            position=handle.tell()
            if not handle.readline():
                break
            result.append(position)
    return result


def row_at(handle,position):
    handle.seek(position)
    return json.loads(handle.readline())


def memory_bytes():
    value=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    return value if sys.platform=='darwin' else value*1024


def fit(args):
    import mlx.core as mx
    import mlx.nn as nn
    import mlx.optimizers as optim
    from mlx.utils import tree_flatten,tree_map,tree_unflatten
    from .model import ToolPolicy,action_loss
    dataset=Path(args.dataset)
    data=json.loads((dataset/'manifest.json').read_text())
    if data.get('format')!='rustosrs-policy-dataset' or data.get('version')!=1:
        raise ValueError('Unsupported policy dataset')
    for split,expected in data['sha256'].items():
        if digest(dataset/f'{split}.jsonl')!=expected:
            raise ValueError(f'Dataset changed after freezing: {split}')
    train_offsets=offsets(dataset/'train.jsonl')
    validation_offsets=offsets(dataset/'validation.jsonl')
    if not train_offsets:
        raise ValueError('Dataset has no training rows')
    root=Path(args.out)
    root.mkdir(parents=True,exist_ok=True)
    config={'dims':160,'layers':4,'heads':4,'context':data['context']}
    mx.random.seed(args.seed)
    if hasattr(mx,'set_cache_limit'):
        mx.set_cache_limit(128*1024*1024)
    if hasattr(mx,'set_memory_limit'):
        mx.set_memory_limit(1536*1024*1024)
    model=ToolPolicy(**config)
    optimizer=optim.AdamW(learning_rate=3e-4,weight_decay=0.01)
    mx.eval(model.parameters())
    start=0
    best=None
    history=[]
    if args.resume:
        checkpoint,bundle=resolve_bundle(root,latest=True)
        if bundle['dataset_sha256']!=digest(dataset/'manifest.json') or bundle['config']!=config or bundle['seed']!=args.seed:
            raise ValueError('Resume requires the same frozen dataset, configuration, and seed')
        model.load_weights(str(checkpoint/'weights.safetensors'))
        optimizer.state=tree_unflatten(list(mx.load(str(checkpoint/'optimizer.safetensors')).items()))
        start=bundle['step']
        best=bundle.get('best_validation_loss')
        history=json.loads((checkpoint/'history.json').read_text())
    elif (root/'latest.json').exists():
        raise ValueError('This run already exists; choose Resume or a new output directory')
    parameter_count=sum(p.size for _,p in tree_flatten(model.trainable_parameters()))
    emit(stage='fit',parameters=parameter_count,step=start,message='Training an experimental tool-action transformer')
    loss_grad=nn.value_and_grad(model,action_loss)
    stop=[False]
    signal.signal(signal.SIGTERM,lambda *_:stop.__setitem__(0,True))
    signal.signal(signal.SIGINT,lambda *_:stop.__setitem__(0,True))
    started=time.monotonic()

    def save(step,validation_loss):
        nonlocal best
        directory=root/f'checkpoint-{step:08d}'
        if directory.exists():
            return
        directory.mkdir()
        model.save_weights(str(directory/'weights.safetensors'))
        mx.save_safetensors(str(directory/'optimizer.safetensors'),dict(tree_flatten(optimizer.state)))
        write(directory/'history.json',history)
        write(directory/'palettes.json',data.get('style_palettes',{}))
        improved=validation_loss is not None and (best is None or validation_loss<best)
        if improved:
            best=validation_loss
        bundle={'format':'rustosrs-tool-policy','version':1,'status':'experimental','learned_policy':True,
                'target_visible_at_inference':False,'config':config,'seed':args.seed,'step':step,
                'parameters':parameter_count,'dataset_sha256':digest(dataset/'manifest.json'),
                'split_algorithm':data['split_algorithm'],'groups':data['groups'],'examples':data['examples'],
                'validation_loss':validation_loss,'best_validation_loss':best,
                'peak_rss_bytes':memory_bytes(),'peak_mlx_bytes':mx.get_peak_memory(),
                'runtime':'mlx==0.32.4; native Apple Silicon','sha256':{name:digest(directory/name) for name in ['weights.safetensors','optimizer.safetensors','history.json','palettes.json']}}
        write(directory/'bundle.json',bundle)
        atomic_reference(root/'latest.json',{'checkpoint':directory.name})
        if improved:
            atomic_reference(root/'best.json',{'checkpoint':directory.name})
        emit(stage='checkpoint',step=step,bundle=str(directory),validation_loss=validation_loss,
             peak_rss_bytes=memory_bytes(),peak_mlx_bytes=mx.get_peak_memory(),message='Experimental checkpoint saved')

    with (dataset/'train.jsonl').open('rb') as training,(dataset/'validation.jsonl').open('rb') as validation:
        order=[]
        for step in range(start,args.steps):
            epoch,index=divmod(step,len(train_offsets))
            if index==0 or not order:
                order=train_offsets.copy()
                random.Random(args.seed+epoch).shuffle(order)
            row=row_at(training,order[index])
            loss,grads=loss_grad(model,row)
            norm=mx.sqrt(sum(mx.sum(g*g) for _,g in tree_flatten(grads)))
            scale=mx.minimum(1.0,1.0/mx.maximum(norm,1e-8))
            grads=tree_map(lambda g:g*scale,grads)
            optimizer.update(model,grads)
            mx.eval(loss,model.parameters(),optimizer.state)
            value=float(loss.item())
            if not math.isfinite(value):
                raise ValueError('Non-finite training loss; use the last valid checkpoint')
            history.append({'step':step+1,'training_loss':value})
            if (step+1)%10==0 or step==start:
                emit(stage='fit',step=step+1,total=args.steps,loss=value,seconds=time.monotonic()-started,message='Learning tool actions')
            if args.cancel_file and Path(args.cancel_file).exists():
                stop[0]=True
            checkpoint=(step+1)%args.checkpoint_every==0 or step+1==args.steps or stop[0] or memory_bytes()>args.memory_mb*1024*1024
            if checkpoint:
                values=[]
                for position in validation_offsets[:32]:
                    values.append(float(action_loss(model,row_at(validation,position)).item()))
                val=sum(values)/len(values) if values else None
                save(step+1,val)
                if memory_bytes()>args.memory_mb*1024*1024:
                    raise ValueError('Memory target exceeded; checkpoint retained. Close other windows or use a smaller dataset context.')
            if stop[0]:
                emit(stage='cancelled',message='Stopped safely after saving a resumable checkpoint')
                return
    emit(stage='ready',bundle=str(root),step=args.steps,message='Policy fitted. Held-out rollouts are required before recommending it.')
