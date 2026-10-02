"""Loopback CONNECT relay for a test process's actual network outage.

Only official Rinx hosts on 443 are reachable. TLS remains end to end: this
relay neither decrypts nor logs requests, headers, payloads or credentials.
Blocking closes this relay's existing sockets, leaving all other apps alone.
"""
import select,socket,socketserver,threading

class Relay:
    def __init__(self):
        self.blocked=threading.Event();self.lock=threading.Lock();self.sockets=set();self.connections=0;self.denied=0
        relay=self
        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                incoming=self.request;outgoing=None
                try:
                    incoming.settimeout(8);header=b''
                    while b'\r\n\r\n' not in header and len(header)<8192:
                        chunk=incoming.recv(1024)
                        if not chunk:return
                        header+=chunk
                    first=header.split(b'\r\n',1)[0].decode('ascii');method,target,_=first.split(' ',2)
                    if method!='CONNECT' or target not in {'matrix.rinx.chat:443','auth.matrix.rinx.chat:443'}:
                        incoming.sendall(b'HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n');return
                    with relay.lock:
                        if relay.blocked.is_set():relay.denied+=1;incoming.sendall(b'HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n');return
                        relay.connections+=1
                    outgoing=socket.create_connection((target.split(':')[0],443),timeout=8)
                    with relay.lock:
                        if relay.blocked.is_set():relay.denied+=1;return
                        relay.sockets.update((incoming,outgoing))
                    incoming.sendall(b'HTTP/1.1 200 Connection Established\r\n\r\n');incoming.setblocking(False);outgoing.setblocking(False)
                    while not relay.blocked.is_set():
                        readable,_,_=select.select([incoming,outgoing],[],[],.2)
                        for source in readable:
                            content=source.recv(65536)
                            if not content:return
                            destination=outgoing if source is incoming else incoming
                            destination.setblocking(True);destination.settimeout(8);destination.sendall(content);destination.setblocking(False)
                except (OSError,ValueError):pass
                finally:
                    with relay.lock:
                        relay.sockets.discard(incoming);relay.sockets.discard(outgoing)
                    if outgoing:outgoing.close()
        class Server(socketserver.ThreadingTCPServer):daemon_threads=True;allow_reuse_address=True
        self.server=Server(('127.0.0.1',0),Handler);self.port=self.server.server_address[1]
        self.thread=threading.Thread(target=self.server.serve_forever,daemon=True);self.thread.start()
    @property
    def url(self):return 'http://127.0.0.1:'+str(self.port)
    def block(self):
        self.blocked.set()
        with self.lock:
            for connection in tuple(self.sockets):
                try:connection.shutdown(socket.SHUT_RDWR)
                except OSError:pass
    def unblock(self):self.blocked.clear()
    def close(self):self.block();self.server.shutdown();self.server.server_close();self.thread.join(timeout=2)
