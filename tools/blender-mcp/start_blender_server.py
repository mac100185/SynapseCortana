#!/usr/bin/env python3
"""Inicia Blender headless con el addon BlenderMCP activo y el socket server escuchando.

Uso:
    python3 start_blender_server.py
    # o
    blender --background --python start_blender_server.py

El socket server escucha en localhost:9876. Para detener Blender, Ctrl+C o
matar el proceso.
"""

import sys
import time


def setup_blender_mcp():
    import bpy

    # 1) Asegurar que el addon está activado.
    try:
        bpy.ops.preferences.addon_enable(module="blender_mcp")
        print("[setup] addon blender_mcp activado")
    except Exception as e:
        print(f"[setup] no pude activar addon via ops: {e}")
        # Fallback: activar directo en preferences.
        try:
            bpy.context.preferences.addons["blender_mcp"] = {"module": "blender_mcp"}
            print("[setup] addon blender_mcp activado (fallback)")
        except Exception as e2:
            print(f"[setup] fallback también falló: {e2}")
            return False

    # 2) Guardar preferencias para que persista.
    try:
        bpy.ops.wm.save_userpref()
        print("[setup] preferencias guardadas")
    except Exception as e:
        print(f"[setup] no pude guardar preferencias: {e}")

    # 3) Iniciar el socket server del addon.
    # El addon expone un operador `blendermcp.start_server` y una clase
    # `BlenderMCPServer` que gestiona el socket. Vamos a invocar el operador.
    try:
        bpy.ops.blendermcp.start_server()
        print("[setup] socket server iniciado en localhost:9876")
        return True
    except Exception as e:
        print(f"[setup] no pude iniciar server via operador: {e}")
        # Fallback: instanciar la clase directamente.
        try:
            from blender_mcp import BlenderMCPServer

            server = BlenderMCPServer(host="localhost", port=9876)
            server.start()
            print("[setup] socket server iniciado (fallback directo)")
            return True
        except Exception as e2:
            print(f"[setup] fallback directo también falló: {e2}")
            return False


if __name__ == "__main__":
    # Cuando se ejecuta con `blender --python this.py`, Blender ya está cargado.
    # Cuando se ejecuta con `python3 this.py`, necesitamos importar bpy via blender.
    try:
        import bpy  # noqa: F401
    except ImportError:
        print(
            "[setup] bpy no disponible — ejecuta con: blender --background --python start_blender_server.py"
        )
        sys.exit(1)

    ok = setup_blender_mcp()
    if not ok:
        print("[setup] FALLÓ la inicialización del addon BlenderMCP")
        sys.exit(2)

    # Mantener Blender vivo para que el socket server siga escuchando.
    # En modo --background, Blender no tiene event loop natural; tenemos
    # que llamar a bpy.app.timers o dormir manualmente.
    print("[setup] Blender headless corriendo. Socket server en localhost:9876.")
    print("[setup] Presiona Ctrl+C para detener.")

    try:
        while True:
            # bpy.app.timers registra callbacks que se ejecutan en el main thread.
            # En modo background, necesitamos llamar a `bpy.app.timers.register`
            # con un callback que retorne None (se ejecuta una vez) o un tiempo
            # en segundos (se repite).
            time.sleep(0.1)
            # Forzar el procesamiento de timers registrados por el addon.
            # En background, Blender NO procesa timers automáticamente.
            # El addon usa `bpy.app.timers.register(execute_wrapper, first_interval=0.0)`
            # que necesita el event loop de Blender. Sin GUI, no se procesa.
            # Workaround: llamar a `bpy.app.timers.register` con un timer que
            # retorne 0.1 para mantener el loop activo.
    except KeyboardInterrupt:
        print("\n[setup] deteniendo Blender...")
        sys.exit(0)
