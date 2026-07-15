os.execute("mkdir -p /state/project /state/code-server /state/misc")
os.execute("chown -Rh coder:coder /state/project /state/code-server /state/misc")
local settings = os.getenv("CODE_SERVER_SETTINGS")
local code_server_config = io.open("/state/code-server-settings.json", "w")
if code_server_config == nil then
  error("Failed to open config file")
end
code_server_config:write(settings)
code_server_config:close()
os.execute("killall code-server nix-daemon 2> /dev/null")
os.execute("nix-daemon > /var/log/nix.log 2>&1 &")
os.execute(
  'su coder -c "mkdir -p /home/coder/.local/share; ln -sf /state/project /home/coder/; ln -sf /state/code-server /home/coder/.local/share/"')
os.execute(
  'su coder -c "mkdir -p /home/coder/.local/share/code-server/Machine; ln -sf /state/code-server-settings.json /home/coder/.local/share/code-server/Machine/settings.json"')
os.execute('su coder -c "code-server --bind-addr 0.0.0.0:9000 --disable-telemetry" > /var/log/code-server.log 2>&1 &')
local password = nil
while password == nil do
  local configfile = io.open("/home/coder/.config/code-server/config.yaml")
  if configfile ~= nil then
    local config = configfile:read("a")
    password = config:match("\npassword: ([a-zA-Z0-9]+)\n")
  end
  os.execute("sleep 1")
end
io.write(password)
