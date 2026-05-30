import os
import posixpath
import socket
import sys
import time
from pathlib import Path

import paramiko

HOST = "43.250.173.119"
USER = "root"
PASSWORD = os.environ.get("SENGOO_DEPLOY_PASSWORD")
LOCAL_ROOT = Path("website/sengoo-official").resolve()
REMOTE_BASE = "/var/www/sengoo-official"
RELEASE = time.strftime("%Y%m%d%H%M%S")
REMOTE_RELEASE = f"{REMOTE_BASE}/releases/{RELEASE}"
NGINX_AVAILABLE = "/etc/nginx/sites-available/sengoo-official"
NGINX_ENABLED = "/etc/nginx/sites-enabled/sengoo-official"
NGINX_CONFD = "/etc/nginx/conf.d/sengoo-official.conf"
SERVER_BLOCK = r'''
server {
    listen 80;
    listen [::]:80;
    server_name www.sengoo.top sengoo.top;

    root /var/www/sengoo-official/current;
    index index.html;

    access_log /var/log/nginx/sengoo-official.access.log;
    error_log /var/log/nginx/sengoo-official.error.log;

    location / {
        try_files $uri $uri/ /index.html;
    }

    location ~* \.(css|js|png|jpg|jpeg|gif|svg|ico|webp)$ {
        expires 7d;
        add_header Cache-Control "public, max-age=604800";
        try_files $uri =404;
    }
}
'''.strip() + "\n"

if not PASSWORD:
    print("SENGOO_DEPLOY_PASSWORD is required", file=sys.stderr)
    sys.exit(2)
if not LOCAL_ROOT.exists():
    print(f"Local site root missing: {LOCAL_ROOT}", file=sys.stderr)
    sys.exit(2)

client = paramiko.SSHClient()
client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
client.connect(
    HOST,
    username=USER,
    password=PASSWORD,
    timeout=20,
    banner_timeout=20,
    auth_timeout=20,
)


def run(cmd, check=True):
    stdin, stdout, stderr = client.exec_command(cmd, timeout=180)
    code = stdout.channel.recv_exit_status()
    out = stdout.read().decode("utf-8", errors="replace")
    err = stderr.read().decode("utf-8", errors="replace")
    print(f"$ {cmd}\nexit={code}")
    if out.strip():
        print(out.strip())
    if err.strip():
        print(err.strip(), file=sys.stderr)
    if check and code != 0:
        raise RuntimeError(f"command failed ({code}): {cmd}\n{err}")
    return code, out, err

run("mkdir -p /var/www/sengoo-official/releases")
run(f"mkdir -p {REMOTE_RELEASE}")

sftp = client.open_sftp()

def mkdir_p(path):
    parts = []
    current = path
    while current not in ("", "/"):
        parts.append(current)
        current = posixpath.dirname(current)
    for item in reversed(parts):
        try:
            sftp.stat(item)
        except FileNotFoundError:
            sftp.mkdir(item)

for local in LOCAL_ROOT.rglob("*"):
    if local.name == "README.md":
        continue
    rel = local.relative_to(LOCAL_ROOT).as_posix()
    remote = f"{REMOTE_RELEASE}/{rel}"
    if local.is_dir():
        mkdir_p(remote)
    else:
        mkdir_p(posixpath.dirname(remote))
        sftp.put(str(local), remote)

sftp.close()
run(f"find {REMOTE_RELEASE} -type f -maxdepth 4 -print")
run(f"ln -sfn {REMOTE_RELEASE} {REMOTE_BASE}/current")
run(f"chmod -R a+rX {REMOTE_RELEASE}")

code, _, _ = run("command -v nginx", check=False)
if code != 0:
    run("if command -v apt-get >/dev/null 2>&1; then apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y nginx; else echo 'nginx missing and apt-get unavailable' >&2; exit 1; fi")

code, _, _ = run("test -d /etc/nginx/sites-available && test -d /etc/nginx/sites-enabled", check=False)
if code == 0:
    config_path = NGINX_AVAILABLE
    escaped = SERVER_BLOCK.replace("'", "'\\''")
    run(f"cat > {config_path} <<'NGINX_CONF'\n{SERVER_BLOCK}NGINX_CONF")
    run(f"ln -sfn {config_path} {NGINX_ENABLED}")
else:
    config_path = NGINX_CONFD
    run(f"cat > {config_path} <<'NGINX_CONF'\n{SERVER_BLOCK}NGINX_CONF")

run("nginx -t")
run("systemctl enable nginx >/dev/null 2>&1 || true")
run("systemctl reload nginx || systemctl restart nginx || nginx -s reload")
run("curl -I --max-time 10 http://127.0.0.1/ -H 'Host: www.sengoo.top'")
print(f"DEPLOYED_RELEASE={REMOTE_RELEASE}")
client.close()
