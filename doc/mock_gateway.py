#!/usr/bin/env python3
"""Mock minimal del gateway OpenClaw para reproducir el error
de handshake del frontend. Levanta un WS en :18790, acepta conexión,
envía el `connect.challenge` legítimo y mide qué pasa.

Uso:
  python3 mock_gateway.py [puerto]
"""
import asyncio
import json
import sys
import websockets


async def handle(ws):
    print("[mock] cliente conectado")
    # 1) Enviar un challenge legítimo con ts actual.
    challenge = {
        "type": "event",
        "event": "connect.challenge",
        "payload": {"nonce": "test-nonce-1234", "ts": 1700000000000},
    }
    await ws.send(json.dumps(challenge))
    print("[mock] challenge enviado")

    # 2) Esperar connect.
    try:
        raw = await asyncio.wait_for(ws.recv(), timeout=5)
        msg = json.loads(raw)
        print(f"[mock] recibí: type={msg.get('type')}, method={msg.get('method')}")
        if msg.get("type") == "req" and msg.get("method") == "connect":
            # Responder con hello-ok
            hello = {
                "type": "res",
                "id": msg.get("id"),
                "ok": True,
                "payload": {
                    "type": "hello-ok",
                    "protocol": 4,
                    "server_version": "mock-1.0",
                    "conn_id": "mock-conn-1",
                },
            }
            await ws.send(json.dumps(hello))
            print("[mock] hello-ok enviado")
            # Esperar un momento y cerrar
            await asyncio.sleep(1)
    except asyncio.TimeoutError:
        print("[mock] no recibí connect en 5s")


async def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18790
    async with websockets.serve(handle, "127.0.0.1", port):
        print(f"[mock] WS server en ws://127.0.0.1:{port}/")
        await asyncio.Future()  # run forever


asyncio.run(main())
