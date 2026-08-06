#!/usr/bin/env python3
import subprocess
import sys
import urllib.parse

if len(sys.argv) < 2:
    print("Usage: atlas-redirect-protocol.py <URL>")
    sys.exit(1)

parsed_url = urllib.parse.urlparse(sys.argv[1])
parsed_query = urllib.parse.parse_qs(parsed_url.query)
url = ""
if parsed_url.scheme == "vscode" and parsed_url.netloc == "vscode.git" and parsed_url.path == "/clone" and "url" in parsed_query and len(parsed_query["url"]) == 1:
    url = parsed_query["url"][0]
else:
    print("Invalid URL")
    sys.exit(1)
url = urllib.parse.quote(url, safe="")
url = f"web+atlasrepo://open?repo={url}"
_ = subprocess.run(["xdg-open", url], check=True)
