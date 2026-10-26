# Builds patrone-remote-desktop's proxy plugin and sign-in helper. The package
# build passes Debian's CFLAGS and LDFLAGS.
CC ?= cc
CFLAGS ?= -O2 -g
CFLAGS += -std=gnu11 -Wall -Wextra
PKG_CONFIG ?= pkg-config
# FreeRDP's own headers mark many old APIs deprecated.
PROXY_CFLAGS = $(shell $(PKG_CONFIG) --cflags freerdp-server-proxy3 freerdp3 winpr3) \
	-Wno-deprecated-declarations
PROXY_LIBS = $(shell $(PKG_CONFIG) --libs freerdp3 winpr3)

all: proxy-patrone-login-plugin.so patrone-remote-desktop-auth

proxy-patrone-login-plugin.so: src/patrone-login.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) -fPIC -shared $(LDFLAGS) -o $@ $< $(PROXY_LIBS)

patrone-remote-desktop-auth: src/auth.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(LDFLAGS) -o $@ $< -lpam

# The tests build the plugin again with a test socket for the helper.
TEST_SOCKET = "/tmp/patrone-rd-test.sock"

tests/proxy-patrone-login-plugin-test.so: src/patrone-login.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) -DAUTH_SOCKET='$(TEST_SOCKET)' -fPIC -shared \
		$(LDFLAGS) -o $@ $< $(PROXY_LIBS)

tests/test-plugin: tests/test-plugin.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) $(LDFLAGS) -o $@ $< $(PROXY_LIBS) -ldl

check: tests/proxy-patrone-login-plugin-test.so tests/test-plugin patrone-remote-desktop-auth
	tests/run.sh tests/proxy-patrone-login-plugin-test.so tests/test-plugin \
		./patrone-remote-desktop-auth

clean:
	rm -f proxy-patrone-login-plugin.so patrone-remote-desktop-auth
	rm -f tests/proxy-patrone-login-plugin-test.so tests/test-plugin

.PHONY: all check clean
