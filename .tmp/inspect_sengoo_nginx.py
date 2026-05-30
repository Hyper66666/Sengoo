import os, paramiko, sys
host='43.250.173.119'; user='root'; password=os.environ['SENGOO_DEPLOY_PASSWORD']
client=paramiko.SSHClient(); client.set_missing_host_key_policy(paramiko.AutoAddPolicy()); client.connect(host, username=user, password=password, timeout=20, banner_timeout=20, auth_timeout=20)
cmd="grep -RIn --include='*.conf' --include='*' 'sengoo.top' /etc/nginx/sites-enabled /etc/nginx/sites-available /etc/nginx/conf.d 2>/dev/null || true"
stdin, stdout, stderr = client.exec_command(cmd, timeout=60)
code=stdout.channel.recv_exit_status()
print(stdout.read().decode('utf-8', errors='replace'))
err=stderr.read().decode('utf-8', errors='replace')
if err: print(err, file=sys.stderr)
client.close()
