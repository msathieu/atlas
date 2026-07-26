import os
import re
import subprocess
import time


def run_command(cmd: str):
    _ = subprocess.run(cmd, shell=True, check=True)


os.makedirs("/state/project", exist_ok=True)
os.makedirs("/state/code-server", exist_ok=True)
os.makedirs("/state/misc", exist_ok=True)
run_command("chown -Rh coder:coder /state/project /state/code-server /state/misc")
with open("/state/code-server-settings.json", "w") as file:
    _ = file.write(os.getenv("CODE_SERVER_SETTINGS", "{}"))
_ = subprocess.run("killall code-server nix-daemon 2> /dev/null", check=False, shell=True)
_ = subprocess.run("nix-daemon > /var/log/nix.log 2>&1 &", check=False, shell=True)
run_command('su coder -c "mkdir -p /home/coder/.local/share; ln -sf /state/project /home/coder/; ln -sf /state/code-server /home/coder/.local/share/"')
run_command('su coder -c "mkdir -p /home/coder/.local/share/code-server/Machine; ln -sf /state/code-server-settings.json /home/coder/.local/share/code-server/Machine/settings.json"')
run_command('su coder -c "SSH_AUTH_SOCK=/run/ssh.sock code-server --bind-addr 0.0.0.0:9000 --disable-telemetry" > /var/log/code-server.log 2>&1 &')
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
