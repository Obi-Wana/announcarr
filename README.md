# announcarr
Announce API content to IRC.

Announcarr is a Rust-based application designed to facilitate the announcement of UNIT3D API content to an IRC channel.

# Configuration
## config.toml Example
```
[app]
announced_file = "announced.log"

[irc]
nickname = "Nick"
password = "Server Pass"
server = "Server URL"
port = 6697
channel = "#Channel"

[api]
url = "API URL"
token = "API TOKEN"
```
