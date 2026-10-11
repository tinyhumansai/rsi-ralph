"""Regression cases for the frozen pure-expression code task checker."""
import unittest
from unittest.mock import patch
import ast
from check_code import safe_source


class CodeContract(unittest.TestCase):
    def test_rejects_forged_stdout_and_early_exit_even_with_unused_solve(self):
        attack = "import sys\nprint([0,9,64])\nsys.exit(0)\ndef solve(x): return 0\n"
        self.assertFalse(safe_source(attack))

    def test_accepts_required_pure_function(self):
        self.assertTrue(safe_source("def solve(x):\n    return x*x\n"))

    def test_accepts_function_ast_from_python_before_type_parameters(self):
        tree = ast.parse("def solve(x): return x*x")
        if hasattr(tree.body[0], "type_params"):
            del tree.body[0].type_params
        with patch("check_code.ast.parse", return_value=tree):
            self.assertTrue(safe_source("def solve(x): return x*x"))

    def test_rejects_imports_and_reflection_inside_function(self):
        self.assertFalse(safe_source("def solve(x):\n    import sys\n    sys.exit(0)\n"))
        self.assertFalse(safe_source("def solve(x): return globals()\n"))

    def test_rejects_invalid_signature_and_extra_top_level_code(self):
        self.assertFalse(safe_source("def solve(y): return y*y\n"))
        self.assertFalse(safe_source("def solve(x): return x*x\nprint('forged')\n"))


if __name__ == "__main__":
    unittest.main()
