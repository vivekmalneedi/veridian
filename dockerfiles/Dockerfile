FROM centos:7

LABEL maintainer="vivekmalneedi@gmail.com"

WORKDIR /root

RUN yum -y update; yum clean all;
RUN yum -y install git gcc; yum clean all;

# install rust toolchain
RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- --default-toolchain stable -y

ENV PATH=/root/.cargo/bin:$PATH
