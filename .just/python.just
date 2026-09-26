# Lints every Python file; formatting is dprint's.
check-python:
    ruff check

# Fixes what Ruff can.
fix-python:
    ruff check --fix
