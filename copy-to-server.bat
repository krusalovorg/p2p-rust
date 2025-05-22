@echo off
echo Creating directory on server...
ssh root@212.109.220.163 "mkdir -p /root/p2p-server/storage-peer"

echo Copying files to server...
scp deploy-docker.sh root@212.109.220.163:/root/p2p-server/
scp target/release/P2P-Server root@212.109.220.163:/root/p2p-server/
scp config.toml root@212.109.220.163:/root/p2p-server/
scp signal_servers.json root@212.109.220.163:/root/p2p-server/

echo Making scripts executable...
ssh root@212.109.220.163 "chmod +x /root/p2p-server/deploy-docker.sh"

echo Files copied successfully!
echo You can now run the deployment script on the server with:
echo ssh root@212.109.220.163 "cd /root/p2p-server && ./deploy-docker.sh" 