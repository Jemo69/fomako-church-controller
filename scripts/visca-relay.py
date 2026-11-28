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
  GET  /api/stream?ip=&user=&pass=   live multipart JPEG (ffmpeg → 30 fps)
  GET  /api/snapshot?ip=&user=&pass= one JPEG (fallback)
  GET  /api/scan?subnet=192.168.1   (TCP 5678 + :80 title + UDP 52381 + ONVIF)
Everything else serves static files from --serve-web (or index text).
"""
import argparse
import concurrent.futures as futures
import json
import os
import shutil
import socket
import ssl
import struct
import subprocess
import sys
import time
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

SEQ = [1]


def sony_wrap(payload: bytes) -> bytes:
    SEQ[0] += 1
    return struct.pack(">HHI", 0x0100, len(payload), SEQ[0]) + payload


class Handler(BaseHTTPRequestHandler):
    server_version = "FoMaKoRelay/0.1"

    def log_message(self, fmt, *a):
        import re

        msg = fmt % a
        # Never log passwords (they arrive in /api/stream, /api/snapshot and /visca query/body).
        msg = re.sub(r'"pass(word)?"\s*:\s*"[^"]*"', '"pass":"***"', msg)
        msg = re.sub(r"pass=[^&\s]*", "pass=***", msg)
        sys.stderr.write("[%s] %s\n" % (time.strftime("%H:%M:%S"), msg))
        sys.stderr.flush()

    def _json(self, obj, code=200):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
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
        if url.path == "/api/snapshot":
            q = urllib.parse.parse_qs(url.query)
            ip = (q.get("ip", [""])[0] or "").strip()
            user = (q.get("user", [""])[0] or "").strip()
            pw = (q.get("pass", [""])[0] or "").strip()
            parts = ip.split(".")
            if len(parts) != 4 or not all(p.isdigit() and 0 <= int(p) <= 255 for p in parts):
                return self._json({"error": "ip like 192.168.5.163"}, 400)
            try:
                port = int((q.get("port", ["554"])[0] or "554"))
            except ValueError:
                port = 554
            jpg, snapshot_error = grab_snapshot(ip, port, user, pw)
            if jpg is None:
                return self._json({"error": snapshot_error or "snapshot failed (camera offline? ffmpeg missing?)"}, 502)
            self.send_response(200)
            self.send_header("Content-Type", "image/jpeg")
            self.send_header("Content-Length", str(len(jpg)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(jpg)
            return
        if url.path == "/api/stream":
            q = urllib.parse.parse_qs(url.query)
            ip = (q.get("ip", [""])[0] or "").strip()
            user = (q.get("user", [""])[0] or "").strip()
            pw = (q.get("pass", [""])[0] or "").strip()
            parts = ip.split(".")
            if len(parts) != 4 or not all(p.isdigit() and 0 <= int(p) <= 255 for p in parts):
                return self._json({"error": "ip like 192.168.5.163"}, 400)
            try:
                port = int((q.get("port", ["554"])[0] or "554"))
            except ValueError:
                port = 554
            self.send_response(200)
            self.send_header("Content-Type", "multipart/x-mixed-replace; boundary=fomako")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Connection", "close")
            self.end_headers()
            frames = stream_jpegs(ip, port, user, pw)
            try:
                for jpg in frames:
                    self.wfile.write(
                        b"\r\n--fomako\r\nContent-Type: image/jpeg\r\nContent-Length: "
                        + str(len(jpg)).encode()
                        + b"\r\n\r\n"
                    )
                    self.wfile.write(jpg)
                    self.wfile.write(b"\r\n")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass
            finally:
                frames.close()
            return
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
                    reply, _ = s.recvfrom(64)
                    ack_payload = reply[8:] if len(reply) >= 8 and reply[:2] == b"\x01\x00" else reply
                    if len(ack_payload) >= 3 and ack_payload[0] == 0x90 and ack_payload[1] == 0x41:
                        s.close()
                        return self._json({"ok": False, "error": f"camera rejected command ({ack_payload.hex(' ')})"}, 502)
                    msg = f"sent {len(payload)}B ack {len(reply)}B"
                except socket.timeout:
                    msg = f"sent {len(payload)}B (no ACK)"
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


def _userinfo(user: str, pw: str) -> str:
    import urllib.parse

    if not user:
        return ""
    # Quote everything unsafe; '@' and ':' must never pass through raw.
    q = lambda s: urllib.parse.quote(s, safe="-_.~!$&'()*+,;=")
    return f"{q(user)}:{q(pw)}@"


def grab_http_snapshot(ip: str, user: str = "", pw: str = "", timeout: int = 5):
    """Prefer the camera's lightweight JPEG endpoint (works without RTSP)."""
    handlers = []
    if user:
        manager = urllib.request.HTTPPasswordMgrWithDefaultRealm()
        manager.add_password(None, f"http://{ip}", user, pw)
        handlers.append(urllib.request.HTTPDigestAuthHandler(manager))
    opener = urllib.request.build_opener(*handlers)
    last_error = "HTTP snapshot unavailable"
    for path in ("/action_snapshot", "/snapshot.jpg"):
        request = urllib.request.Request(
            f"http://{ip}{path}?t={time.time_ns()}",
            headers={"Cache-Control": "no-cache", "User-Agent": "FoMaKoController/0.4"},
        )
        try:
            with opener.open(request, timeout=timeout) as response:
                data = response.read(8 * 1024 * 1024)
                if data.startswith(b"\xff\xd8\xff"):
                    return data, ""
                last_error = "camera returned a non-JPEG snapshot"
        except urllib.error.HTTPError as error:
            last_error = f"HTTP snapshot {error.code}"
        except Exception as error:
            last_error = f"HTTP snapshot failed: {type(error).__name__}"
    return None, last_error


def grab_snapshot(ip: str, port: int = 554, user: str = "", pw: str = "", timeout: int = 15):
    """One JPEG frame from HTTP stills, with RTSP fallback for older firmware."""
    import shutil
    import subprocess

    jpg, http_error = grab_http_snapshot(ip, user, pw)
    if jpg is not None:
        return jpg, ""
    if shutil.which("ffmpeg") is None:
        return None, f"{http_error}; ffmpeg not found on the relay PC"
    rtsp_error = "RTSP unavailable"
    for path in ("2", "live/av1", "1", "live/av0"):
        try:
            p = subprocess.run(
                ["ffmpeg", "-hide_banner", "-loglevel", "error",
                 "-rtsp_transport", "tcp", "-timeout", "5000000",
                 "-i", f"rtsp://{_userinfo(user, pw)}{ip}:{port}/{path}",
                 "-skip_frame", "nokey",
                 "-frames:v", "1", "-q:v", "5",
                 "-f", "image2pipe", "-vcodec", "mjpeg", "-"],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout,
            )
        except subprocess.TimeoutExpired:
            rtsp_error = "RTSP timed out — check camera IP/WiFi"
            continue
        except Exception as e:
            rtsp_error = f"could not start ffmpeg: {type(e).__name__}"
            break
        if p.returncode == 0 and p.stdout:
            return p.stdout, ""
        err = (p.stderr or b"").decode("utf-8", "replace")
        if "401" in err or "Unauthorized" in err:
            rtsp_error = "RTSP 401 Unauthorized — check camera login"
        elif "404" in err or "Not Found" in err:
            rtsp_error = "RTSP stream not found"
        elif "timed out" in err.lower() or "timeout" in err.lower():
            rtsp_error = "RTSP timed out — check camera IP/WiFi"
        else:
            rtsp_error = "ffmpeg could not decode RTSP — check camera stream/codec"
    return None, f"{http_error}; {rtsp_error}"


def _jpeg_frames(stream, chunk_size: int = 65536):
    """Split a byte stream into JPEGs (SOI..EOI), ignoring any multipart headers."""
    buf = b""
    while True:
        chunk = stream.read(chunk_size)
        if not chunk:
            return
        buf += chunk
        while True:
            start = buf.find(b"\xff\xd8\xff")
            if start < 0:
                buf = buf[-2:]
                break
            if start:
                buf = buf[start:]
            end = buf.find(b"\xff\xd9", 2)
            if end < 0:
                if len(buf) > 16 * 1024 * 1024:
                    buf = b""
                break
            yield buf[: end + 2]
            buf = buf[end + 2 :]


def _rtsp_jpegs(ip: str, port: int, user: str, pw: str):
    """One ffmpeg per RTSP path; stops after the first path that produced frames."""
    import shutil
    import subprocess

    if shutil.which("ffmpeg") is None:
        return
    for path in ("2", "live/av1", "1", "live/av0"):
        cmd = [
            "ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin",
            "-rtsp_transport", "tcp", "-timeout", "5000000",
            "-i", f"rtsp://{_userinfo(user, pw)}{ip}:{port}/{path}",
            "-an", "-sn", "-dn", "-vf", "fps=30,scale=480:-2",
            "-f", "mjpeg", "-q:v", "5", "-",
        ]
        try:
            proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, bufsize=0)
        except OSError:
            return
        saw = False
        try:
            for jpg in _jpeg_frames(proc.stdout):
                saw = True
                yield jpg
        finally:
            proc.kill()
            proc.wait()
        if saw:
            return


def _snapshot_jpegs(ip: str, user: str, pw: str) -> bool:
    """Poll HTTP stills flat out; returns True if any frame was served."""
    failures = 0
    saw = False
    while failures < 3:
        jpg, _ = grab_http_snapshot(ip, user, pw, timeout=4)
        if jpg is None:
            failures += 1
            time.sleep(0.8)
            continue
        failures = 0
        saw = True
        yield jpg
    return saw


def stream_jpegs(ip: str, port: int = 554, user: str = "", pw: str = ""):
    """Endless JPEG frames: RTSP at the camera's rate, HTTP stills otherwise."""
    while True:
        saw = False
        for jpg in _rtsp_jpegs(ip, port, user, pw):
            saw = True
            yield jpg
        if not saw:
            # RTSP unavailable (or ffmpeg missing): stills as fast as they come.
            saw = yield from _snapshot_jpegs(ip, user, pw)
        time.sleep(0.25 if saw else 3.0)


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


def detect_lan_ip() -> str:
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.connect(("8.8.8.8", 80))
        ip = s.getsockname()[0]
        s.close()
        if ip and not ip.startswith("127."):
            return ip
    except OSError:
        pass
    return "127.0.0.1"


def ensure_self_signed_cert(lan_ip: str):
    cache_dir = Path.home() / ".cache" / "fomako-relay"
    cache_dir.mkdir(parents=True, exist_ok=True)
    cert = cache_dir / f"cert-{lan_ip}.pem"
    key = cache_dir / f"key-{lan_ip}.pem"
    if cert.is_file() and key.is_file():
        return cert, key
    if shutil.which("openssl") is None:
        raise RuntimeError("--https needs openssl on PATH (or pass --cert and --key)")
    san = f"subjectAltName=DNS:localhost,IP:127.0.0.1,IP:{lan_ip}"
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout", str(key), "-out", str(cert),
            "-days", "365", "-subj", f"/CN={lan_ip}",
            "-addext", san,
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return cert, key


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--serve-web", default="")
    ap.add_argument("--https", action="store_true", help="Serve over HTTPS (required by Android Chrome WebGPU on LAN IPs)")
    ap.add_argument("--cert", default="", help="TLS certificate PEM path (implies --https)")
    ap.add_argument("--key", default="", help="TLS private key PEM path")
    args = ap.parse_args()

    class Server(ThreadingHTTPServer):
        pass

    srv = Server(("0.0.0.0", args.port), Handler)
    srv.web_dir = Path(args.serve_web) if args.serve_web else None
    lan_ip = detect_lan_ip()
    scheme = "http"
    if args.https or args.cert:
        if args.cert and args.key:
            cert, key = Path(args.cert), Path(args.key)
        else:
            cert, key = ensure_self_signed_cert(lan_ip)
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        ctx.load_cert_chain(certfile=str(cert), keyfile=str(key))
        srv.socket = ctx.wrap_socket(srv.socket, server_side=True)
        scheme = "https"

    relay_enc = urllib.parse.quote(f"{scheme}://{lan_ip}:{args.port}", safe="")
    subnet = ".".join(lan_ip.split(".")[:3])
    print(f"relay on :{args.port} ({scheme})  web={srv.web_dir or '(api only)'}")
    print(f"phone -> {scheme}://{lan_ip}:{args.port}/#relay={relay_enc}&subnet={subnet}&autostart=scan")
    if scheme == "https":
        print("note: first visit on Android Chrome will show a self-signed cert prompt (Advanced -> Proceed).")
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    sys.exit(main())
