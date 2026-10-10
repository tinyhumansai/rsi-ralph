"""Host-owned checker for a pure integer arithmetic function artifact."""
import ast
import pathlib
import subprocess
import sys
import uuid

IMAGE = "python@sha256:2d9aefe2fef018a7eb2c13064c89c71929800fd2e5dccdbf52ea5da5bb8d929a"


def safe_source(source):
    """Accept only one solve(x) function returning a bounded arithmetic expression."""
    if len(source) > 4096:
        return False
    tree = ast.parse(source)
    if len(tree.body) != 1 or not isinstance(tree.body[0], ast.FunctionDef):
        return False
    fn = tree.body[0]
    args = fn.args
    if (fn.name != "solve" or fn.decorator_list or fn.returns or fn.type_params
            or args.posonlyargs or args.kwonlyargs or args.defaults
            or args.kw_defaults or args.vararg or args.kwarg
            or len(args.args) != 1 or args.args[0].arg != "x"
            or args.args[0].annotation or len(fn.body) != 1
            or not isinstance(fn.body[0], ast.Return) or fn.body[0].value is None):
        return False
    nodes = list(ast.walk(fn.body[0].value))
    allowed = (ast.BinOp, ast.UnaryOp, ast.Add, ast.Sub, ast.Mult,
               ast.FloorDiv, ast.Mod, ast.UAdd, ast.USub, ast.Name,
               ast.Load, ast.Constant)
    return len(nodes) <= 64 and all(
        isinstance(node, allowed)
        and (not isinstance(node, ast.Name) or node.id == "x")
        and (not isinstance(node, ast.Constant)
             or type(node.value) is int and abs(node.value) <= 1000000)
        for node in nodes)



def check(path):
    """Reject malformed source, then compare private outputs outside Docker."""
    try:
        if not safe_source(pathlib.Path(path).read_text()):
            return False
    except (OSError, SyntaxError, UnicodeError):
        return False
    name = "tinyrsi-" + uuid.uuid4().hex
    args = ["/usr/bin/docker", "run", "--rm", "--name", name,
            "--network", "none", "--read-only", "--cap-drop", "ALL",
            "--security-opt", "no-new-privileges", "--memory", "128m",
            "--pids-limit", "32", "--cpus", "1", "--user", "65534:65534",
            "--mount", "type=bind,src=" + str(pathlib.Path(path).resolve()) +
            ",dst=/artifact,readonly", IMAGE, "python", "-I", "-c",
            'import runpy; f=runpy.run_path("/artifact")["solve"]; '
            'print([f(x) for x in [0,3,8]])']
    try:
        result = subprocess.run(args, capture_output=True, timeout=5)
        return result.returncode == 0 and result.stdout.strip() == b"[0, 9, 64]"
    except (subprocess.TimeoutExpired, OSError):
        return False
    finally:
        subprocess.run(["/usr/bin/docker", "rm", "-f", name],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                       timeout=3)


if __name__ == "__main__":
    sys.exit(0 if check(sys.argv[1]) else 1)
