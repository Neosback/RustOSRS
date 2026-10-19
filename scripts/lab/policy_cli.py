#!/usr/bin/env python3
"""Native local tool-policy jobs. Stdlib preparation; isolated MLX fitting and inference."""
import argparse
import json
from pathlib import Path
import subprocess
import sys


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest='command',required=True)
    setup=sub.add_parser('setup')
    setup.add_argument('--runtime',required=True)
    prepare=sub.add_parser('prepare')
    prepare.add_argument('--prefabs')
    prepare.add_argument('--corpus')
    prepare.add_argument('--out',required=True)
    prepare.add_argument('--binary',default='target/debug/osrs-lab')
    prepare.add_argument('--context',type=int,default=1024)
    prepare.add_argument('--max-buildings',type=int,default=0)
    fit=sub.add_parser('fit')
    fit.add_argument('--dataset',required=True)
    fit.add_argument('--out',required=True)
    fit.add_argument('--steps',type=int,default=2000)
    fit.add_argument('--seed',type=int,default=7)
    fit.add_argument('--checkpoint-every',type=int,default=100)
    fit.add_argument('--memory-mb',type=int,default=2048)
    fit.add_argument('--resume',action='store_true')
    rollout=sub.add_parser('rollout')
    rollout.add_argument('--bundle',required=True)
    rollout.add_argument('--task',required=True)
    rollout.add_argument('--goal',required=True)
    rollout.add_argument('--binary',default='target/debug/osrs-lab')
    rollout.add_argument('--out',required=True)
    rollout.add_argument('--steps',type=int,default=256)
    rollout.add_argument('--seed',type=int,default=7)
    rollout.add_argument('--temperature',type=float,default=0.6)
    evaluate=sub.add_parser('evaluate')
    evaluate.add_argument('--bundle',required=True)
    evaluate.add_argument('--dataset',required=True)
    evaluate.add_argument('--binary',default='target/debug/osrs-lab')
    evaluate.add_argument('--out',required=True)
    evaluate.add_argument('--split',choices=['validation','test'],default='validation')
    evaluate.add_argument('--episodes',type=int,default=24)
    evaluate.add_argument('--steps',type=int,default=256)
    for command_parser in [setup,prepare,fit,rollout,evaluate]:
        command_parser.add_argument('--cancel-file')
    args=parser.parse_args()
    if getattr(args,'steps',1)<1 or getattr(args,'checkpoint_every',1)<1 or not 64<=getattr(args,'context',1024)<=2048:
        parser.error('Steps/checkpoint interval must be positive and context must be 64..2048')
    if hasattr(args,'binary'):
        args.binary=str(Path(args.binary).resolve())
    if args.command=='setup':
        runtime=Path(args.runtime).resolve()
        subprocess.run([sys.executable,'-m','venv',str(runtime)],check=True)
        subprocess.run([str(runtime/'bin/python'),'-m','pip','install','-r',str(Path(__file__).parent/'policy/requirements.txt')],check=True)
        print(json.dumps({'stage':'ready','python':str(runtime/'bin/python'),'message':'Local MLX runtime installed'}),flush=True)
    elif args.command=='prepare':
        from policy.data import prepare
        prepare(args)
    elif args.command=='fit':
        from policy.fit import fit
        fit(args)
    elif args.command=='rollout':
        from policy.rollout import rollout
        rollout(args)
    else:
        from policy.rollout import evaluate
        evaluate(args)


if __name__=='__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'stage':'failed','message':str(error)}),flush=True)
        sys.exit(1)
