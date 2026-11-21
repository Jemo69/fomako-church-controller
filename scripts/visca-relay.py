#!/usr/bin/env python3
"""FoMaKo VISCA relay: lets phone/browser builds control cameras.

Browsers can't open raw TCP/UDP sockets, so the web build talks to this
tiny relay on your LAN PC (same machine that serves the web UI), and the
relay forwards VISCA to the cameras. Stdlib only, no pip packages.

  python3 scripts/visca-relay.py --port 8080 --serve-web dist/web

Then on the phone browser open:  http://<this-PC-LAN-IP>:8080
In the app: Cameras -> turn Relay ON, URL http://<this-PC-LAN-IP>:8080,
then Scan network or drive cameras.

Endpoints (same origin, so no CORS headaches):
  POST /visca     {"ip","port","proto":"sony_udp"|"tcp","payload":"hex"}
  GET  /api/scan?subnet=192.168.1   (TCP 5678 + :80 title + UDP 52381 + ONVIF)
Everything else serves static files from --serve-web (or index text).
"""
import argparse
import concurrent.futures as futures
import json
import socket
import struct
import sys
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

SEQ = [1]


def sony_wrap(payload: bytes) -> bytes:
    SEQ[0] += 1
    return struct.pack(">HHI", 0x0100, len(payload), SEQ[0]) + payload


class Handler(BaseHTTPRequestHandler):
    server_version = "FoMaKoRelay/0.1"

    def log_message(self, fmt, *a):
        sys.stderr.write("[%s] %s\n" % (time.strftime("%H:%M:%S"), fmt % a))
        sys.stderr.flush()

    def _json(self, obj, code=200):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        url = urllib.parse.urlparse(self.path)
        if url.path == "/api/scan":
            q = urllib.parse.parse_qs(url.query)
            prefix = (q.get("subnet", [""])[0] or "").strip()
            parts = prefix.split(".")
            if len(parts) == 4 and parts[3] == "0":
                prefix = ".".join(parts[:3])  # also accept "192.168.1.0"
            if not valid_prefix(prefix):
                return self._json({"error": "subnet like 192.168.1"}, 400)
            return self._json({"found": scan_subnet(prefix)})
        # static files for the phone web UI
        web = self.server.web_dir
        if web and (url.path in ("/", "/index.html")):
            idx = web / "index.html"
            if idx.exists():
                return self._serve(idx)
        if web and url.path.startswith("/") and ".." not in url.path:
            f = web / url.path.lstrip("/")
            if f.is_file():
                return self._serve(f)
        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(b"FoMaKo VISCA relay OK. Serve the web UI with --serve-web dist/web")

    def _serve(self, f: Path):
        data = f.read_bytes()
        ctype = "application/octet-stream"
        if f.suffix == ".html":
            ctype = "text/html"
        elif f.suffix == ".js":
            ctype = "text/javascript"
        elif f.suffix == ".wasm":
            ctype = "application/wasm"
        elif f.suffix == ".css":
            ctype = "text/css"
        elif f.suffix == ".png":
            ctype = "image/png"
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if self.path != "/visca":
            return self._json({"error": "use POST /visca"}, 404)
        try:
            ln = int(self.headers.get("Content-Length", "0"))
            req = json.loads(self.rfile.read(max(ln, 0)) or b"{}")
            ip = req["ip"]
            port = int(req["port"])
            proto = req["proto"]
            payload = bytes.fromhex(req["payload"])
        except Exception as e:
            return self._json({"error": f"bad request: {e}"}, 400)
        try:
            if proto == "sony_udp":
                s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                s.settimeout(1.0)
                s.sendto(sony_wrap(payload), (ip, port))
                try:
                    n, _ = s.recvfrom(64)
                    msg = f"sent {len(payload)}B ack {n}B"
                except socket.timeout:
                    msg = f"sent {len(payload)}B (no ack, ok on LAN)"
                s.close()
            else:
                s = socket.create_connection((ip, port), timeout=1.5)
                s.settimeout(1.0)
                s.sendall(payload)
                s.close()
                msg = f"sent {len(payload)}B tcp"
            return self._json({"ok": True, "msg": msg})
        except Exception as e:
            return self._json({"ok": False, "error": str(e)[:200]}, 502)


def valid_prefix(p: str) -> bool:
    parts = p.split(".")
    if len(parts) != 3:
        return False
    try:
        return all(0 <= int(x) <= 255 for x in parts)
    except ValueError:
        return False


def tcp_open(ip: str, port: int, ms: int = 350) -> bool:
    s = socket.socket()
    s.settimeout(ms / 1000)
    try:
        s.connect((ip, port))
        return True
    except OSError:
        return False
    finally:
        s.close()


def http_hint(ip: str) -> str:
    try:
        s = socket.create_connection((ip, 80), timeout=0.4)
        s.settimeout(0.6)
        s.sendall(f"GET / HTTP/1.0\r\nHost: {ip}\r\nConnection: close\r\n\r\n".encode())
        data = b""
        while len(data) < 16384:
            try:
                chunk = s.recv(4096)
            except socket.timeout:
                break
            if not chunk:
                break
            data += chunk
        s.close()
        text = data.decode("utf-8", "ignore").lower()
        hint = ""
        if "<title>" in text:
            hint = text.split("<title>", 1)[1].split("</title>", 1)[0].strip()[:64]
        for key in ("fomako", "ptz", "ip camera", "network camera"):
            if key in text:
                hint = (hint + " +" + key).strip(" +") if hint else key
                break
        return hint
    except OSError:
        return ""


def visca_udp_ok(ip: str) -> bool:
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.settimeout(0.45)
        s.sendto(sony_wrap(bytes([0x81, 0x09, 0x04, 0x00, 0xFF])), (ip, 52381))
        data, _ = s.recvfrom(128)
        s.close()
        return len(data) >= 8
    except OSError:
        return False


def probe(ip: str):
    t5678 = tcp_open(ip, 5678)
    hint = http_hint(ip)
    web = bool(hint) or tcp_open(ip, 80, 250)
    u52381 = visca_udp_ok(ip) if (t5678 or web) else False
    if t5678 or u52381 or hint:
        return {"ip": ip, "tcp5678": t5678, "udp52381": u52381, "http_hint": hint, "onvif": False}
    return None


def onvif_probe_ms(ms: int = 2500):
    msg = (
        '<?xml version="1.0" encoding="UTF-8"?>'
        '<e:Envelope xmlns:e="http://www.w3.org/2003/05/soap-envelope" '
        'xmlns:w="http://schemas.xmlsoap.org/ws/2004/08/addressing" '
        'xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" '
        'xmlns:dn="http://www.onvif.org/ver10/network/wsdl">'
        "<e:Header><w:MessageID>uuid:fomako-relay</w:MessageID>"
        "<w:To>urn:schemas-xmlsoap-org:ws:2005:04:discovery</w:To>"
        "<w:Action>http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</w:Action>"
        "</e:Header><e:Body><d:Probe><d:Types>dn:NetworkVideoTransmitter</d:Types>"
        "</d:Probe></e:Body></e:Envelope>"
    ).encode()
    found = []
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.settimeout(0.5)
        try:
            s.sendto(msg, ("239.255.255.250", 3702))
        except OSError:
            return found
        end = time.time() + ms / 1000
        seen = set()
        buf = bytearray(65535)
        while time.time() < end:
            try:
                n, _ = s.recvfrom_into(buf)
            except socket.timeout:
                continue
            text = bytes(buf[:n]).decode("utf-8", "ignore").lower()
            for marker in ("http://", "https://"):
                rest = text
                while marker in rest:
                    host = rest.split(marker, 1)[1]
                    host = "".join(c for c in host[:64] if c.isalnum() or c in ".-:").split()[0]
                    ip = host.split(":")[0]
                    parts = ip.split(".")
                    if len(parts) == 4 and all(p.isdigit() and 0 <= int(p) <= 255 for p in parts):
                        if ip not in seen:
                            seen.add(ip)
                            found.append({"ip": ip, "tcp5678": False, "udp52381": False, "http_hint": "", "onvif": True})
                    rest = rest.split(marker, 1)[1][1:]
        s.close()
    except OSError:
        pass
    return found


def scan_subnet(prefix: str):
    found = {c["ip"]: c for c in onvif_probe_ms()}
    ips = [f"{prefix}.{i}" for i in range(1, 255)]
    with futures.ThreadPoolExecutor(max_workers=32) as ex:
        for hit in ex.map(probe, ips):
            if hit:
                found.setdefault(hit["ip"], hit)
                if hit["ip"] in found and found[hit["ip"]].get("onvif") and not hit.get("onvif"):
                    base = found[hit["ip"]]
                    base.update({k: v for k, v in hit.items() if v})
    return sorted(found.values(), key=lambda c: [int(x) for x in c["ip"].split(".")])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--serve-web", default="")
    args = ap.parse_args()

    class Server(ThreadingHTTPServer):
        pass

    srv = Server(("0.0.0.0", args.port), Handler)
    srv.web_dir = Path(args.serve_web) if args.serve_web else None
    print(f"relay on :{args.port}  web={srv.web_dir or '(api only)'}")
    print("phone -> http://<this-PC-IP>:%d   (relay URL identical)" % args.port)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    sys.exit(main())
