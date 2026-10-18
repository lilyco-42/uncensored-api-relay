#!/usr/bin/env bash
# 一条命令部署到服务器：编译 Linux 二进制 -> 上传 -> 装 systemd
set -euo pipefail

SERVER="${1:-root@lain42.top}"
REMOTE_DIR="/opt/uncensored-proxy"
TOOLCHAIN_BIN='C:/Users/liuqi/.rustup/toolchains/stable-x86_64-pc-windows-msvc/bin'

cd "$(dirname "$0")/.."
export PATH="$TOOLCHAIN_BIN:$PATH"

echo '[1/4] 编译 Linux 二进制'
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu

echo '[2/4] 上传二进制与 systemd 单元'
ssh "$SERVER" "mkdir -p $REMOTE_DIR"
scp target/x86_64-unknown-linux-gnu/release/uncensored-proxy "$SERVER:$REMOTE_DIR/"
scp deploy/uncensored-proxy.service "$SERVER:/etc/systemd/system/"

echo '[3/4] 安装并启动'
ssh "$SERVER" "chmod +x $REMOTE_DIR/uncensored-proxy && systemctl daemon-reload && systemctl enable --now uncensored-proxy"

echo '[4/4] 健康检查'
ssh "$SERVER" "systemctl --no-pager status uncensored-proxy | head -8"
echo "完成。域名侧请配置 lain42.top 反代到 127.0.0.1:8080。"
