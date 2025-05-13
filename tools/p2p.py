import socket
import threading
import random
import json
import logging
import time

def dat_to_bytes(diction: dict) -> bytes:
    return json.dumps(diction).encode("utf-8")

def bytes_to_dat(byte: bytes) -> dict:
    return json.loads(byte.decode("utf-8"))

def randomport():
    return random.randint(16000, 65535)

def STUN(port, host="stun.ekiga.net"):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("0.0.0.0", port))
    sock.setblocking(0)
    server = socket.gethostbyname(host)
    work = True
    while work:
        sock.sendto(
            b"\x00\x01\x00\x00!\x12\xa4B\xd6\x85y\xb8\x11\x030\x06xi\xdfB",
            (server, 3478),
        )
        for i in range(20):
            try:
                ans, addr = sock.recvfrom(2048)
                work = False
                break
            except:
                time.sleep(0.01)

    sock.close()
    return socket.inet_ntoa(ans[28:32]), int.from_bytes(ans[26:28], byteorder="big")

class Session:
    def __init__(self):
        self.local_port = randomport()
        self.public_ip, self.public_port = STUN(self.local_port)
        self.socket = None
        self.client = None
        self.thread = None
        logging.info(f'"{self.public_ip}",{self.public_port}')

    def make_connection(self, ip, port, timeout=10):
        logging.debug(f"Start waiting for handshake with {ip}:{port}")
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(("0.0.0.0", self.local_port))
        sock.setblocking(0)
        while True:
            sock.sendto(b"Con. Request!", (ip, port))
            time.sleep(2)
            try:
                ans, addr = sock.recvfrom(9999)
                sock.sendto(b"Con. Request!", (ip, port))
                sock.close()
                sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                sock.bind(("0.0.0.0", self.local_port))
                sock.setblocking(0)
                self.client = (ip, port)
                self.socket = sock
                logging.debug(f"Hole with {self.client} punched!")
                break
            except Exception as e:
                assert timeout > 0
                timeout -= 1
                logging.debug(f"No handshake with {ip}:{port} yet...")

    def backlife_cycle(self, freq=1):
        th = threading.Thread(target=self.life_cycle, args=(freq,))
        th.start()
        self.thread = th
        logging.warning(f"Session with {self.client} stabilized!")

    def life_cycle(self, freq=1):
        while 1:
            self.socket.sendto(b"KPL", self.client)  # Keep-alive
            time.sleep(max(random.gauss(1 / freq, 3), 0))

            while True:
                try:
                    ans, reply_addr = self.socket.recvfrom(9999)
                    logging.debug(
                        f"{self.client[0]}: Recieved {ans[:3].decode('utf-8')} from {reply_addr}: {ans}"
                    )
                except:
                    break

                if ans[:3] == b"KPL":
                    continue
                elif ans[:3] == b"MSG":
                    message = bytes_to_dat(ans[3:])
                    print(f"Message from {self.client[0]}: {message['text']}")

    def send_message(self, message):
        self.socket.sendto(b"MSG" + dat_to_bytes({"text": message}), self.client)

if __name__ == "__main__":
    logging.basicConfig(level=logging.DEBUG)
    s = Session()
    print(f"Send this to your friend {s.public_ip}:{s.public_port}")
    ip, port = input("Input the IP and port from your friend (format: IP:port): ").split(":")
    s.make_connection(ip, int(port))
    s.backlife_cycle(1)
    while True:
        message = input("Enter your message: ")
        s.send_message(message)