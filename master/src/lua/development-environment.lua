os.execute("mkdir -p /state/project /state/code-server /state/misc")
os.execute("chown -Rh coder:coder /state/project /state/code-server /state/misc")
os.execute(
  'su coder -c "mkdir -p /home/coder/.local/share; ln -s /state/project /home/coder/project; ln -s /state/code-server /home/coder/.local/share/code-server"')
os.execute("killall -9 code-server 2> /dev/null")
os.execute('su coder -c "code-server --bind-addr 0.0.0.0:9000 --disable-telemetry" > /var/log/code-server.log 2>&1 &')
os.execute("sleep 3")
local configfile = io.open("/home/coder/.config/code-server/config.yaml")
if configfile == nil then
  error("Can't open code-server config")
end
local config = configfile:read("a")
local password = config:match("\npassword: ([a-zA-Z0-9]+)\n")
io.write(password)
