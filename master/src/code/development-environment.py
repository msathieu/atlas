import os
import re
import subprocess
import time
import urllib.error
import urllib.request


def run_command(cmd: str | list[str], user: str | None = None, shell: bool = True):
    _ = subprocess.run(cmd, shell=shell, check=True, user=user, group=user, extra_groups=[], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


os.makedirs("/state/project", exist_ok=True)
os.makedirs("/state/ide", exist_ok=True)
os.makedirs("/state/misc", exist_ok=True)
run_command("chown -Rh ide:ide /state/project /state/ide /state/misc")
with open("/state/ide-settings.json", "w") as file:
    _ = file.write(os.getenv("DEVELOPMENT_ENVIRONMENT_SETTINGS", "{}"))
with open("/etc/gitconfig", "w") as file:
    _ = file.write(os.getenv("DEVELOPMENT_ENVIRONMENT_GITCONFIG", "{}"))
_ = subprocess.run("killall code-server nix-daemon 2> /dev/null", check=False, shell=True)
run_command("nix-daemon > /var/log/nix.log 2>&1 &")
run_command("mkdir -p /state/ide/Machine && ln -sf /state/ide-settings.json /state/ide/Machine/settings.json", "ide")
for i in os.getenv("DEVELOPMENT_ENVIRONMENT_PATHS", "").split(";"):
    if i:
        run_command(f"mkdir -p /state/misc/{i} && rm -f /home/ide/{i} && ln -sf /state/misc/{i} /home/ide/{i}", "ide")
if os.getenv("DEVELOPMENT_ENVIRONMENT_REPO") and not os.path.exists("/state/project/.git"):
    if os.getenv("DEVELOPMENT_ENVIRONMENT_BRANCH"):
        run_command(["git", "clone", os.getenv("DEVELOPMENT_ENVIRONMENT_REPO") or "", "-b", os.getenv("DEVELOPMENT_ENVIRONMENT_BRANCH") or "", "/state/project"], "ide", False)
    else:
        run_command(["git", "clone", os.getenv("DEVELOPMENT_ENVIRONMENT_REPO") or "", "/state/project"], "ide", False)
nix_command = ""
if os.path.exists("/state/project/flake.lock"):
    nix_command = "nix develop /state/project -c"
run_command(
    f'su ide -c "SSH_AUTH_SOCK=/run/ssh.sock {nix_command} code-server --user-data-dir /state/ide --bind-addr 0.0.0.0:9000 --disable-telemetry --proxy-domain localhost /state/project" > /var/log/code-server.log 2>&1 &'
)
for i in os.getenv("DEVELOPMENT_ENVIRONMENT_EXTENSIONS", "").split(";"):
    if i:
        run_command(f"HOME=/home/ide code-server --user-data-dir /state/ide --install-extension {i}", "ide")
password = None
while password is None:
    try:
        with open("/home/ide/.config/code-server/config.yaml", "r") as file:
            match = re.search(r"^password: ([a-zA-Z0-9]+)$", file.read(), re.MULTILINE)
            if match:
                password = match.group(1)
    except FileNotFoundError:
        pass
    if password is None:
        time.sleep(1)
print(password, end="")
while True:
    try:
        with urllib.request.urlopen("http://localhost:9000", timeout=2) as response:
            if response.status == 200:
                break
    except urllib.error.URLError, TimeoutError:
        pass
    time.sleep(1)
