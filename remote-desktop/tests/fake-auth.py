#!/usr/bin/env python3
"""Answers like patrone-remote-desktop's sign-in helper, for test-plugin.

Expects the account alice, from 192.0.2.7. Its password decides the answer:
"right" signs in to port 21000 with the secret 0123abcd, "busy" is signed in
elsewhere, "garbage" gets a malformed answer, and anything else is denied.
"""

import os
import socket
import sys

ANSWERS = {"right": "ok 21000 0123abcd", "busy": "busy", "garbage": "ok notaport"}

path = sys.argv[1]
if os.path.exists(path):
    os.unlink(path)
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen()

while True:
    connection, _ = server.accept()
    with connection:
        request = b""
        while chunk := connection.recv(4096):
            request += chunk
        user, password, client = (request.split(b"\0") + [b"", b"", b""])[:3]
        if user == b"alice" and client == b"192.0.2.7":
            answer = ANSWERS.get(password.decode(), "denied")
        else:
            answer = "denied"
        connection.sendall(answer.encode() + b"\n")
