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

# Run
## Build
```
cargo build --release
```

## Systemd service
It is advisable to execute this application as a systemd service: `/etc/systemd/system/announcarr.service`

```
[Unit]
Description=announcarr
After=network.target

[Service]
Type=simple
ExecStart=/path/to/announcarr/target/release/announcarr
WorkingDirectory=/path/to/announcarr/
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
Alias=announcarr.service
```

## Start/Stop
```
systemctl start announcarr.service

systemctl stop announcarr.service
```
