os.execute("ln -s /state/project /home/coder/project")
os.execute('su coder -c "code-server --bind-addr 0.0.0.0:9000 --disable-telemetry" > /var/log/code-server.log 2>&1 &')
