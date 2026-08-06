import os
import re
import subprocess
import time
import urllib.error
import urllib.request


def run_command(cmd: str | list[str], user: str | None = None, shell: bool = True):
    _ = subprocess.run(cmd, shell=shell, check=True, user=user, group=user, extra_groups=[], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


os.makedirs("/state/project", exist_ok=True)
os.makedirs("/state/code-server", exist_ok=True)
os.makedirs("/state/misc", exist_ok=True)
run_command("chown -Rh coder:coder /state/project /state/code-server /state/misc")
with open("/state/code-server-settings.json", "w") as file:
    _ = file.write(os.getenv("DEVELOPMENT_ENVIRONMENT_SETTINGS", "{}"))
_ = subprocess.run("killall code-server nix-daemon 2> /dev/null", check=False, shell=True)
_ = subprocess.run("nix-daemon > /var/log/nix.log 2>&1 &", check=False, shell=True)
run_command("mkdir -p /state/code-server/Machine; ln -sf /state/code-server-settings.json /state/code-server/Machine/settings.json", "coder")
if os.getenv("DEVELOPMENT_ENVIRONMENT_REPO") and not os.path.exists("/state/project/.git"):
    if os.getenv("DEVELOPMENT_ENVIRONMENT_BRANCH"):
        run_command(["git", "clone", os.getenv("DEVELOPMENT_ENVIRONMENT_REPO") or "", "-b", os.getenv("DEVELOPMENT_ENVIRONMENT_BRANCH") or "", "/state/project"], "coder", False)
    else:
        run_command(["git", "clone", os.getenv("DEVELOPMENT_ENVIRONMENT_REPO") or "", "/state/project"], "coder", False)
nix_command = ""
if os.path.exists("/state/project/flake.lock"):
    nix_command = "nix develop /state/project -c"
run_command(
    f'su coder -c "SSH_AUTH_SOCK=/run/ssh.sock {nix_command} code-server --user-data-dir /state/code-server --bind-addr 0.0.0.0:9000 --disable-telemetry --proxy-domain localhost /state/project" > /var/log/code-server.log 2>&1 &'
)
password = None
while password is None:
    try:
        with open("/home/coder/.config/code-server/config.yaml", "r") as file:
            match = re.search(r"^password: ([a-zA-Z0-9]+)$", file.read(), re.MULTILINE)
            if match:
                password = match.group(1)
    except FileNotFoundError:
        pass
    if password is None:
        time.sleep(1)
print(password, end="")
for i in os.getenv("DEVELOPMENT_ENVIRONMENT_EXTENSIONS", "").split(","):
    if i:
        run_command(f"HOME=/home/coder code-server --install-extension {i}", "coder")
while True:
    try:
        with urllib.request.urlopen("http://localhost:9000", timeout=2) as response:
            if response.status == 200:
                break
    except urllib.error.URLError, TimeoutError:
        pass
    time.sleep(1)
