#!/bin/sh
set -eu
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export CC=/usr/bin/gcc CXX=/usr/bin/g++
export GOMAXPROCS=2
cd /opt/upstream
mkdir -p logs bin build
apk info -vv > logs/packages.txt
pkg-config --modversion fuse fuse3 libmtp libusb-1.0 > logs/dependency-versions.txt
for project in jmtpfs simple-mtpfs; do
  (
    src=$(find sources -maxdepth 1 -type d -name "$project-*")
    cp -R "$src" "build/$project"
    cd "build/$project"
    autoreconf -fi
    ./configure --prefix=/opt/upstream/install
    make -j1
    cp "src/$project" "/opt/upstream/bin/$project"
  ) > "logs/$project-build.log" 2>&1 && echo "$project PASS" || echo "$project FAIL"
done
(
 src=$(find sources -maxdepth 1 -type d -name 'android-file-transfer-linux-*')
 cmake -S "$src" -B build/aft -DBUILD_QT_UI=OFF -DBUILD_PYTHON=OFF -DBUILD_TAGLIB=OFF -DBUILD_MTPZ=OFF -DBUILD_FUSE=ON -DCMAKE_BUILD_TYPE=Release
 cmake --build build/aft --target aft-mtp-mount -j1
 cp build/aft/fuse/aft-mtp-mount bin/aft-mtp-mount
) > logs/aft-build.log 2>&1 && echo 'aft PASS' || echo 'aft FAIL'
(
 src=$(find sources -maxdepth 1 -type d -name 'go-mtpfs-*')
 cp -R "$src" build/go-mtpfs
 cd build/go-mtpfs
 go version
 go mod download
 go build -p 1 -o /opt/upstream/bin/go-mtpfs .
 go list -m all > /opt/upstream/logs/go-modules.txt
) > logs/go-mtpfs-build.log 2>&1 && echo 'go-mtpfs PASS' || echo 'go-mtpfs FAIL'
sha256sum bin/* > logs/binaries.sha256
