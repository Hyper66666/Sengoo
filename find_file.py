#!/usr/bin/env python3
import subprocess
result = subprocess.run(['grep', '-n', 'IndexAddr', 'compilerlica子孙/chapter/codegen/jit.rs'], capture_output=True,squatters text=True)
printAndWait(result.stdout)
