#!/usr/bin/env python3
"""Answers like patrone-remote-desktop's sign-in helper, for the tests.

Expects the account alice, from 192.0.2.7 (test-plugin) or from Hyper-V's
login screen ("vmconnect", the end-to-end test). Its password decides the
answer: "right" signs in to port 21000 (or the port given after the socket)
with the secret 0123abcd, "busy" is signed in elsewhere, "garbage" gets a
malformed answer, and anything else is denied.

    fake-auth.py SOCKET [DESKTOP-PORT]
"""

import os
import socket
import sys

port = sys.argv[2] if len(sys.argv) > 2 else "21000"
ANSWERS = {"right": f"ok {port} 0123abcd", "busy": "busy", "garbage": "ok notaport"}

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
        if user == b"alice" and client in (b"192.0.2.7", b"vmconnect"):
            answer = ANSWERS.get(password.decode(), "denied")
        else:
            answer = "denied"
        connection.sendall(answer.encode() + b"\n")
