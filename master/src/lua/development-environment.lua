os.execute("mkdir -p /state/project /state/code-server")
os.execute("chown -Rh coder:coder /state/project /state/code-server")
os.execute(
  'su coder -c "mkdir -p /home/coder/.local/share; ln -s /state/project /home/coder/project; ln -s /state/code-server /home/coder/.local/share/code-server"')
os.execute('su coder -c "code-server --bind-addr 0.0.0.0:9000 --disable-telemetry" > /var/log/code-server.log 2>&1 &')
