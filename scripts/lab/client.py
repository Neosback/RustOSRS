"""Python-stdlib client for osrs-lab's JSON-lines tool protocol."""
import json
import subprocess
from pathlib import Path


class LabClient:
    def __init__(self, binary):
        self.process = subprocess.Popen([str(binary), 'serve'], stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, text=True, bufsize=1)

    def call(self, command):
        self.process.stdin.write(json.dumps(command, separators=(',', ':')) + '\n')
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f'Lab server exited: {self.process.poll()}')
        response = json.loads(line)
        if not response['ok']:
            raise RuntimeError(response['error'])
        return response['data']

    def reset(self, task, seed=0, visible=False):
        return self.call({'cmd': 'reset', 'task': str(Path(task).resolve()), 'seed': seed,
                           'mode': 'visible' if visible else 'score_only'})

    def step(self, action, observe=True):
        cmd = {'cmd': 'step', 'action': action}
        if not observe:
            cmd['observe'] = False
        return self.call(cmd)

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=5)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
