#!/usr/bin/env python3
"""Regression coverage for source-derived defaults; does not certify their implementation."""
import unittest
import sys

sys.dont_write_bytecode = True
from audit_hd_config import inventory, method_body


class ConfigInventoryTests(unittest.TestCase):
    def test_inline_methods_stop_at_their_own_brace(self):
        rows = inventory('''
String KEY_ONE = "one";
@ConfigItem(keyName = KEY_ONE)
default boolean first() { return true; }
@ConfigItem(keyName = "two")
default int second() {
    return 50;
}
''')
        self.assertEqual(rows['one']['default_body'], 'return true;')
        self.assertEqual(rows['two']['default_body'], 'return 50;')

    def test_nested_blocks_and_braces_in_literals_or_comments(self):
        body = '''if (true) { /* } */ return "{\\\"}"; }
// }
return '}';'''
        self.assertEqual(method_body(body + '\n} trailing', 0), body)

    def test_unclosed_or_duplicate_methods_are_errors(self):
        with self.assertRaisesRegex(ValueError, 'Unclosed'):
            method_body('return 1;', 0)
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            inventory('@ConfigItem(keyName = "x") default int a() { return 1; } '
                      '@ConfigItem(keyName = "x") default int b() { return 2; }')


if __name__ == '__main__':
    unittest.main()
