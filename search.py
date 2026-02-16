for line in open('compilerleanest/chrollo/chapter/codegen/jit.rs'):
    if 'IndexAddr' in line:
        print(line.rstrip())
