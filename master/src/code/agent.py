import os
import subprocess
import time
import urllib.error
import urllib.request


def run_command(cmd: str | list[str], user: str | None = None, shell: bool = True):
    _ = subprocess.run(cmd, shell=shell, check=True, user=user, group=user, extra_groups=[], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


os.makedirs("/state/data", exist_ok=True)
os.makedirs("/nix/store", exist_ok=True)
run_command("chown -Rh agent:agent /state/data")
_ = subprocess.run("killall hermes nix-daemon 2> /dev/null", check=False, shell=True)
run_command("nix-daemon > /var/log/nix.log 2>&1 &")
run_command("HERMES_HOME=/state/data /home/agent/.local/bin/hermes config set platforms.webhook.enabled true", "agent")
run_command('su agent -c "HERMES_HOME=/state/data AGENT_BROWSER_ARGS=--no-sandbox /home/agent/.local/bin/hermes gateway run" > /var/log/hermes.log 2>&1 &')
run_command(
    'su agent -c "HERMES_HOME=/state/data HERMES_DASHBOARD_BASIC_AUTH_USERNAME=admin AGENT_BROWSER_ARGS=--no-sandbox /home/agent/.local/bin/hermes dashboard --host=0.0.0.0" > /var/log/hermes-dashboard.log 2>&1 &'
)

while True:
    try:
        with urllib.request.urlopen("http://localhost:9119/login", timeout=2) as response:
            if response.status == 200:
                break
    except (urllib.error.URLError, TimeoutError):
        pass
    time.sleep(1)
