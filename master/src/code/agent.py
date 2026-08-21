import os
import subprocess


def run_command(cmd: str | list[str], user: str | None = None, shell: bool = True):
    _ = subprocess.run(cmd, shell=shell, check=True, user=user, group=user, extra_groups=[], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


os.makedirs("/state/data", exist_ok=True)
run_command("chown -Rh hermes:hermes /state/data")
run_command('su hermes -c "HERMES_HOME=/state/data HERMES_DASHBOARD=1 hermes gateway run" > /var/log/hermes.log 2>&1 &')
