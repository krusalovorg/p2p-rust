@echo off
@REM echo Building project...
@REM cargo build --release

echo Creating deployment directory...
ssh root@212.109.220.163 "mkdir -p /root/p2p-server"

echo Copying files to server...
scp target/release/P2P-Server root@212.109.220.163:/root/p2p-server/
scp config.toml root@212.109.220.163:/root/p2p-server/
scp signal_servers.json root@212.109.220.163:/root/p2p-server/

echo Setting up firewall rules...
ssh root@212.109.220.163 "ufw allow 8081/tcp"
ssh root@212.109.220.163 "ufw allow 8080/tcp"
ssh root@212.109.220.163 "ufw allow 3031/tcp"
ssh root@212.109.220.163 "ufw allow 80/tcp"

echo Creating systemd service...
ssh root@212.109.220.163 "cat > /etc/systemd/system/p2p-server.service << 'EOL'
[Unit]
Description=P2P Server
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/p2p-server
ExecStart=/root/p2p-server/P2P-Server --signal
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOL"

echo Enabling and starting service...
ssh root@212.109.220.163 "systemctl daemon-reload && systemctl enable p2p-server && systemctl restart p2p-server"

echo Deployment completed!
echo You can check service status with: ssh root@212.109.220.163 "systemctl status p2p-server"