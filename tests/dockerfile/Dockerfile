FROM fedora:34
ARG use_local_artifacts
ENV USE_LOCAL_ARTIFACTS=${use_local_artifacts:-0}
ENV GOPATH=/go
ENV PATH=$PATH:/go/bin
RUN dnf install -y git make cmake gcc gcc-c++ which iproute iputils procps-ng vim-minimal tmux net-tools htop tar jq npm openssl-devel perl rust cargo golang
# only required for deployment script
RUN npm install -g ts-node && npm install -g typescript

COPY --from=avaplatform/avalanchego:v1.9.4 /avalanchego/ /avalanchego/
RUN mv /avalanchego/build/avalanchego /usr/bin/avalanchego

# the actual source code for this repo, this **only** includes checked in files!
# this is a bit of a pain but it does speed things up a lot
ADD gravity.tar /
# build steps for all codebases in this repo, must be below the add statement

# cleanly builds everything, or if `USE_LOCAL_ARTIFACTS=1` then the gravity
# binary from `build-container.sh` is moved to its correct spot
RUN if [[ "${USE_LOCAL_ARTIFACTS:-0}" -eq "0" ]] ; then \
    pushd /gravity/orchestrator/ \
    && PATH=$PATH:$HOME/.cargo/bin cargo build --all --release \
    && pushd /gravity/module/ \
    && PATH=$PATH:/usr/local/go/bin GOPROXY=https://proxy.golang.org make \
    && PATH=$PATH:/usr/local/go/bin make install \
    && pushd /gravity/solidity/ \
    && npm ci \
; else \
    mv gravity /usr/bin/gravity \
; fi