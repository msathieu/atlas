local file = io.open("/etc/os-release")
if file == nil then
  error("Can't open /etc/os-release")
end
local content = file:read("a")
io.write(content)
