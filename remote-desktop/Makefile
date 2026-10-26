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
WINPR_CFLAGS = $(shell $(PKG_CONFIG) --cflags winpr3) -Wno-deprecated-declarations
WINPR_LIBS = $(shell $(PKG_CONFIG) --libs winpr3)

all: proxy-patrone-login-plugin.so patrone-remote-desktop-auth

proxy-patrone-login-plugin.so: src/patrone-login.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(PROXY_CFLAGS) -fPIC -shared $(LDFLAGS) -o $@ $< $(PROXY_LIBS)

patrone-remote-desktop-auth: src/auth.c
	$(CC) $(CPPFLAGS) $(CFLAGS) $(WINPR_CFLAGS) $(LDFLAGS) -o $@ $< -lpam $(WINPR_LIBS)

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

check: tests/proxy-patrone-login-plugin-test.so tests/test-plugin patrone-remote-desktop-auth \
		tests/auth-remember-test tests/test-sam
	tests/run.sh tests/proxy-patrone-login-plugin-test.so tests/test-plugin \
		./patrone-remote-desktop-auth

clean:
	rm -f proxy-patrone-login-plugin.so patrone-remote-desktop-auth
	rm -f tests/proxy-patrone-login-plugin-test.so tests/test-plugin
	rm -f tests/auth-remember-test tests/test-sam

.PHONY: all check clean
