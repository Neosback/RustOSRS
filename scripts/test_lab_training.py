#!/usr/bin/env python3
"""Offline learner contracts; no network, cache or ML installation required."""
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).parent / 'lab'))

if __name__ == '__main__':
    suite = unittest.defaultTestLoader.discover(str(Path(__file__).parent / 'lab'), 'test_*.py')
    sys.exit(not unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful())
