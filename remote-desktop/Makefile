# Builds patrone-remote-desktop's proxy plugins, its sign-in helper and the
# login screen for Hyper-V's enhanced session. The package build passes
# Debian's CFLAGS and LDFLAGS.
CC ?= cc
CFLAGS ?= -O2 -g
CFLAGS += -std=gnu11 -Wall -Wextra
PKG_CONFIG ?= pkg-config
# FreeRDP's own headers mark many old APIs deprecated.
PROXY_CFLAGS = $(shell $(PKG_CONFIG) --cflags freerdp-server-proxy3 freerdp3 winpr3) \
	-Wno-deprecated-declarations
PROXY_LIBS = $(shell $(PKG_CONFIG) --libs freerdp3 winpr3)
VMCONNECT_LIBS = $(shell $(PKG_CONFIG) --libs freerdp-server-proxy3 freerdp3 winpr3)
WINPR_CFLAGS = $(shell $(PKG_CONFIG) --cflags winpr3) -Wno-deprecated-declarations
WINPR_LIBS = $(shell $(PKG_CONFIG) --libs winpr3)
GREETER_PACKAGES = freerdp-server3 freerdp3 winpr3 pangocairo xkbcommon
GREETER_CFLAGS = $(shell $(PKG_CONFIG) --cflags $(GREETER_PACKAGES)) -Wno-deprecated-declarations
GREETER_LIBS = $(shell $(PKG_CONFIG) --libs $(GREETER_PACKAGES)) -lm

all: proxy-patrone-login-plugin.so proxy-patrone-vmconnect-plugin.so patrone-remote-desktop-auth \
	patrone-remote-desktop-greeter

proxy-patrone-login-plugin.so: src/patrone-login.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) -fPIC -shared $(LDFLAGS) -o $@ $< $(PROXY_LIBS)

proxy-patrone-vmconnect-plugin.so: src/patrone-vmconnect.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) -fPIC -shared $(LDFLAGS) -o $@ $< $(VMCONNECT_LIBS)

patrone-remote-desktop-auth: src/auth.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(WINPR_CFLAGS) $(LDFLAGS) -o $@ $< -lpam $(WINPR_LIBS)

patrone-remote-desktop-greeter: src/greeter.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(GREETER_CFLAGS) $(LDFLAGS) -o $@ $< $(GREETER_LIBS)

# The tests build the plugin again with a test socket for the helper, and the
# helper with a test SAM file that any account may write.
TEST_SOCKET = "/tmp/patrone-rd-test.sock"
TEST_SAM = "/tmp/patrone-rd-test.sam"

tests/proxy-patrone-login-plugin-test.so: src/patrone-login.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) -DAUTH_SOCKET='$(TEST_SOCKET)' -fPIC -shared \
		$(LDFLAGS) -o $@ $< $(PROXY_LIBS)

tests/test-plugin: tests/test-plugin.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) $(LDFLAGS) -o $@ $< $(PROXY_LIBS) -ldl

tests/auth-remember-test: src/auth.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(WINPR_CFLAGS) -DREMEMBER_TEST -DSAM_FILE='$(TEST_SAM)' \
		$(LDFLAGS) -o $@ $< -lpam $(WINPR_LIBS)

tests/test-sam: tests/test-sam.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(WINPR_CFLAGS) $(LDFLAGS) -o $@ $< $(WINPR_LIBS)

# Hyper-V's enhanced session end to end: the proxy with the plugin from here,
# a stand-in desktop and a client that types
tests/test-proxy: tests/test-proxy.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) $(LDFLAGS) -o $@ $< $(VMCONNECT_LIBS) -ldl

tests/test-desktop: tests/test-desktop.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(GREETER_CFLAGS) $(LDFLAGS) -o $@ $< $(GREETER_LIBS)

tests/test-client: tests/test-client.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) $(LDFLAGS) -o $@ $< $(PROXY_LIBS)

check: tests/proxy-patrone-login-plugin-test.so tests/test-plugin patrone-remote-desktop-auth \
		tests/auth-remember-test tests/test-sam proxy-patrone-vmconnect-plugin.so \
		patrone-remote-desktop-greeter tests/test-proxy tests/test-desktop tests/test-client
	tests/run.sh tests/proxy-patrone-login-plugin-test.so tests/test-plugin \
		./patrone-remote-desktop-auth

clean:
	rm -f proxy-patrone-login-plugin.so proxy-patrone-vmconnect-plugin.so
	rm -f patrone-remote-desktop-auth patrone-remote-desktop-greeter
	rm -f tests/proxy-patrone-login-plugin-test.so tests/test-plugin
	rm -f tests/auth-remember-test tests/test-sam
	rm -f tests/test-proxy tests/test-desktop tests/test-client

.PHONY: all check clean
