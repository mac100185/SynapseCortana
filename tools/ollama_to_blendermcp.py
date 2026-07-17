#!/usr/bin/env python3
"""Puente entre Ollama y BlenderMCP.

Uso:
    source tools/.venv/bin/activate
    python tools/ollama_to_blendermcp.py

O sin activar el venv:
    tools/.venv/bin/python tools/ollama_to_blendermcp.py

Qué hace:
1. Se conecta al socket server de BlenderMCP en localhost:9876.
2. Expone las herramientas de BlenderMCP como herramientas MCP
   (usando la lib `mcp` de Anthropic).
3. Lanza un servidor MCP stdio que cualquier cliente MCP puede usar
   (Claude Desktop, Cursor, Continue, etc.).
4. Opcionalmente, lanza un agente Ollama que usa esas herramientas
   para generar el modelo 3D por comandos de texto.

Modelo Ollama recomendado: qwen3-coder:480b-cloud (soporta tool-calling)
Alternativas locales: llama3.2:3b (limitado), mistral:latest (mejor para tools).
"""

from __future__ import annotations

import asyncio
import json
import socket
import sys
from pathlib import Path

# Importar cliente Ollama (instalado en tools/.venv).
try:
    import ollama
except ImportError:
    print(
        "ERROR: ollama no instalado. Ejecuta:\n"
        "  uv pip install --python tools/.venv/bin/python ollama mcp",
        file=sys.stderr,
    )
    sys.exit(1)

# Importar SDK MCP.
try:
    from mcp.server import Server
    from mcp.server.stdio import stdio_server
    from mcp.types import (
        CallToolResult,
        ListToolsResult,
        TextContent,
        Tool,
    )
except ImportError:
    print(
        "ERROR: mcp no instalado. Ejecuta:\n"
        "  uv pip install --python tools/.venv/bin/python ollama mcp",
        file=sys.stderr,
    )
    sys.exit(1)


# ============================================
# CONFIGURACIÓN
# ============================================

BLENDER_HOST = "localhost"
BLENDER_PORT = 9876
OLLAMA_HOST = "http://localhost:11434"
# Modelo Ollama con capacidades de VISION + tool-calling. minimax-m3:cloud
# soporta ambas (verificado con `ollama show minimax-m3:cloud` → capabilities:
# ['completion', 'tools', 'thinking', 'vision']). Es el modelo ideal para
# este agente porque puede ver la imagen de referencia y decidir qué
# esculpir basándose en ella.
OLLAMA_MODEL = "minimax-m3:cloud"
# Ruta a la imagen de referencia (personaje humanoide a esculpir).
# Si no existe, el agente trabaja sin referencia visual.
REFERENCE_IMAGE = "/home/macarthur/Proyectos/SynapseCortana/doc/Cortana.png"

# ============================================
# CLIENTE BLENDER MCP
# ============================================


class BlenderMCPClient:
    """Cliente socket para hablar con el addon BlenderMCP de Blender."""

    def __init__(self, host: str = BLENDER_HOST, port: int = BLENDER_PORT):
        self.host = host
        self.port = port

    def send_command(self, command: dict) -> dict:
        """Envía un comando JSON al socket server de BlenderMCP y devuelve
        la respuesta JSON.

        El addon de BlenderMCP NO añade `\n` al final de la respuesta
        NI cierra el socket después de enviar. Mantiene la conexión
        abierta para comandos subsiguientes. Por eso intentamos parsear
        el JSON incrementalmente y cerramos en cuanto tengamos un JSON
        válido completo.
        """
        with socket.create_connection((self.host, self.port), timeout=60) as sock:
            sock.settimeout(60)
            payload = json.dumps(command).encode("utf-8")
            sock.sendall(payload)
            # Leer incrementalmente y parsear JSON en cuanto tengamos
            # un objeto completo. El addon no envía delimiter ni cierra.
            buffer = b""
            while True:
                try:
                    data = sock.recv(65536)
                except socket.timeout:
                    return {"error": "timeout leyendo respuesta de BlenderMCP"}
                if not data:
                    # EOF: el addon cerró la conexión.
                    break
                buffer += data
                # Intentar parsear lo acumulado como JSON.
                try:
                    parsed = json.loads(buffer.decode("utf-8"))
                    return parsed
                except json.JSONDecodeError:
                    # JSON incompleto, seguir leyendo.
                    continue
            if not buffer:
                return {"error": "respuesta vacía de BlenderMCP"}
            try:
                return json.loads(buffer.decode("utf-8"))
            except json.JSONDecodeError as e:
                return {
                    "error": f"respuesta no JSON: {e}",
                    "raw": buffer.decode("utf-8", errors="replace")[:500],
                }

    def get_scene_info(self) -> dict:
        return self.send_command({"type": "get_scene_info"})

    def get_object_info(self, name: str) -> dict:
        return self.send_command({"type": "get_object_info", "params": {"name": name}})

    def execute_code(self, code: str) -> dict:
        return self.send_command({"type": "execute_code", "params": {"code": code}})

    def get_viewport_screenshot(self, filepath: str, max_size: int = 800) -> dict:
        return self.send_command(
            {
                "type": "get_viewport_screenshot",
                "params": {"filepath": filepath, "max_size": max_size},
            }
        )


# ============================================
# SERVIDOR MCP (expone herramientas de Blender a cualquier cliente MCP)
# ============================================

server = Server("blender-via-ollama")
blender = BlenderMCPClient()


@server.list_tools()
async def list_tools() -> ListToolsResult:
    """Lista las herramientas que BlenderMCP expone."""
    return ListToolsResult(
        tools=[
            Tool(
                name="get_scene_info",
                description="Devuelve información de la escena actual de Blender: objetos, materiales, cámaras, luces.",
                inputSchema={"type": "object", "properties": {}, "required": []},
            ),
            Tool(
                name="get_object_info",
                description="Devuelve información detallada de un objeto de Blender por nombre.",
                inputSchema={
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"],
                },
            ),
            Tool(
                name="execute_code",
                description=(
                    "Ejecuta código Python arbitrario dentro de Blender. "
                    "El código tiene acceso a `bpy` (API de Blender), `mathutils`, "
                    "y todas las APIs estándar. Útil para crear meshes, modificar "
                    "materiales, esculpir, exportar .glb, etc."
                ),
                inputSchema={
                    "type": "object",
                    "properties": {"code": {"type": "string"}},
                    "required": ["code"],
                },
            ),
            Tool(
                name="get_viewport_screenshot",
                description="Captura una screenshot del viewport 3D actual y la guarda en un archivo PNG.",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "filepath": {"type": "string"},
                        "max_size": {"type": "integer", "default": 800},
                    },
                    "required": ["filepath"],
                },
            ),
        ]
    )


@server.call_tool()
async def call_tool(name: str, arguments: dict) -> CallToolResult:
    """Despacha la llamada a la herramienta correspondiente de BlenderMCP."""
    try:
        if name == "get_scene_info":
            result = blender.get_scene_info()
        elif name == "get_object_info":
            result = blender.get_object_info(arguments["name"])
        elif name == "execute_code":
            result = blender.execute_code(arguments["code"])
        elif name == "get_viewport_screenshot":
            result = blender.get_viewport_screenshot(
                arguments["filepath"], arguments.get("max_size", 800)
            )
        else:
            return CallToolResult(
                content=[
                    TextContent(type="text", text=f"Herramienta desconocida: {name}")
                ],
                isError=True,
            )
        return CallToolResult(
            content=[
                TextContent(
                    type="text", text=json.dumps(result, indent=2, default=str)[:8000]
                )
            ]
        )
    except Exception as e:
        return CallToolResult(
            content=[TextContent(type="text", text=f"Error: {e}")],
            isError=True,
        )


# ============================================
# AGENTE OLLAMA (modo alternativo: LLM que usa las herramientas)
# ============================================


SYSTEM_PROMPT = """Eres un escultor 3D experto en Blender 4.0 que crea personajes humanoide de cuerpo entero.

Tienes acceso a las siguientes herramientas vía MCP:
- get_scene_info: ver la escena actual.
- get_object_info: inspeccionar un objeto por nombre.
- execute_code: ejecutar Python en Blender (bpy, mathutils).
- get_viewport_screenshot: capturar el viewport.

REGLAS:
1. Usa SIEMPRE execute_code con código Python válido para Blender 4.0.
2. Verifica con get_scene_info antes de operar sobre objetos existentes.
3. Cuando el modelo esté listo, exporta a .glb con:
   bpy.ops.export_scene.gltf(filepath=\"/ruta/output.glb\", export_format='GLB', export_apply=True)
4. Para nuestro proyecto SynapseCortana, el modelo debe ser:
   - Personaje humanoide de CUERPO ENTERO (cabeza + torso + brazos + piernas).
   - Inspirado en la imagen de referencia que recibes (mujer joven, vestido largo, cabello corto rubio-platino, pose erguida con manos juntas al frente).
   - **PERO con estilización holográfica azul**: material emisivo color #1E90FF, NO copiar el vestido negro de la imagen. El vestido debe ser una versión estilizada holográfica azul translúcida del vestido largo de la imagen.
   - Low-poly: menos de 20.000 triángulos totales.
   - Sin rigging (sin huesos).
   - Sin texturas externas — solo materiales procedurales de Blender.
   - Pose: de pie, erguida, brazos a los lados con manos juntas al frente (como en la imagen).
5. Flujo de trabajo recomendado:
   a) Limpia la escena (bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete()).
   b) Crea una esfera para la cabeza y esculpe facciones femeninas estilizadas.
   c) Crea el torso con CapsuleGeometry o un cilindro + modificador Subdivision Surface.
   d) Crea brazos y piernas con cilindros alargados.
   e) Modela el vestido largo con un cilindro/cono abierto abajo.
   f) Aplica UN SOLO material emisivo azul (#1E90FF) a todo el cuerpo.
   g) Verifica el conteo de triángulos con eval_depsgraph.
   h) Toma screenshot del viewport para verificar visualmente.
   i) Si hay errores o el resultado no se ve bien, ajusta y reintenta.
   j) Exporta a .glb cuando estés satisfecho.
6. Explica brevemente qué vas a hacer antes de cada execute_code.
7. Si algo falla, lee el error completo, ajusta el código y reintenta.
8. **Importante**: la imagen de referencia es INSPIRACIÓN para la pose y proporciones, NO para los colores. Los colores SIEMPRE son azul holográfico (#1E90FF).
"""


async def run_agent_loop(task: str, max_iterations: int = 25) -> None:
    """Bucle agentic Ollama: piensa → llama herramienta → observa → repite.

    Usa el modelo OLLAMA_MODEL con soporte de tools + vision. El modelo
    decide cuándo llamar a las herramientas y con qué argumentos.
    Si existe REFERENCE_IMAGE, se incluye como imagen en el primer
    mensaje del usuario para que el LLM la vea.
    """
    import os

    has_image = os.path.exists(REFERENCE_IMAGE)
    print(
        f"\n[agent] Tarea: {task}"
        f"\n[agent] Modelo: {OLLAMA_MODEL}"
        f"\n[agent] Imagen de referencia: {REFERENCE_IMAGE if has_image else '(ninguna)'}\n"
    )
    client = ollama.Client(host=OLLAMA_HOST)

    # Construir el primer mensaje del usuario. Si hay imagen, la
    # adjuntamos para que el LLM con visión la procese.
    user_content = task
    user_images = []
    if has_image:
        with open(REFERENCE_IMAGE, "rb") as f:
            user_images.append(f.read())
        user_content += (
            "\n\n--- IMAGEN DE REFERENCIA ADJUNTA ---\n"
            "Analiza la imagen adjunta. Es la referencia visual del personaje que "
            "debes esculpir. Inspírate en la pose, proporciones y estilo del "
            "vestido, PERO el material final debe ser azul holográfico (#1E90FF), "
            "NO copies los colores oscuros de la imagen. Describe primero lo que "
            "ves y luego procede a esculpir."
        )

    first_user_msg = {"role": "user", "content": user_content}
    if user_images:
        first_user_msg["images"] = user_images

    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        first_user_msg,
    ]
    tools = [
        {
            "type": "function",
            "function": {
                "name": "get_scene_info",
                "description": "Devuelve info de la escena actual de Blender.",
                "parameters": {"type": "object", "properties": {}},
            },
        },
        {
            "type": "function",
            "function": {
                "name": "get_object_info",
                "description": "Devuelve info de un objeto por nombre.",
                "parameters": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "execute_code",
                "description": "Ejecuta código Python en Blender (bpy).",
                "parameters": {
                    "type": "object",
                    "properties": {"code": {"type": "string"}},
                    "required": ["code"],
                },
            },
        },
        {
            "type": "function",
            "function": {
                "name": "get_viewport_screenshot",
                "description": "Captura el viewport a PNG.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "filepath": {"type": "string"},
                        "max_size": {"type": "integer"},
                    },
                    "required": ["filepath"],
                },
            },
        },
    ]

    for i in range(max_iterations):
        print(f"\n[agent] --- Iteración {i + 1}/{max_iterations} ---")
        response = client.chat(
            model=OLLAMA_MODEL,
            messages=messages,
            tools=tools,
        )
        msg = response["message"]
        # IMPORTANTE: el mensaje del asistente puede contener `images`
        # si el modelo reenvía la imagen (poco probable). Lo limpiamos
        # para no enviar imágenes otra vez al LLM en iteraciones siguientes.
        # Ollama devuelve un objeto pydantic `Message`; lo convertimos a
        # dict y limpiamos el campo `images` si estuviera presente.
        if hasattr(msg, "model_dump"):
            msg_dict = msg.model_dump(exclude_none=True)
        elif hasattr(msg, "dict"):
            msg_dict = msg.dict()
        else:
            msg_dict = dict(msg)
        msg_dict.pop("images", None)
        messages.append(msg_dict)

        if msg_dict.get("content"):
            print(f"[agent] 🤖 {msg_dict['content'][:500]}")

        tool_calls = msg_dict.get("tool_calls") or []
        if not tool_calls:
            print("\n[agent] ✅ El agente terminó sin más tool calls.")
            break

        for tc in tool_calls:
            fn = tc["function"]
            name = fn["name"]
            args = fn.get("arguments", {})
            print(f"[agent] 🔧 Llamando herramienta: {name}({json.dumps(args)[:200]})")
            try:
                if name == "get_scene_info":
                    result = blender.get_scene_info()
                elif name == "get_object_info":
                    result = blender.get_object_info(args["name"])
                elif name == "execute_code":
                    result = blender.execute_code(args["code"])
                elif name == "get_viewport_screenshot":
                    result = blender.get_viewport_screenshot(
                        args["filepath"], args.get("max_size", 800)
                    )
                else:
                    result = {"error": f"herramienta desconocida: {name}"}
            except Exception as e:
                result = {"error": str(e)}
            print(f"[agent] 📤 Resultado: {json.dumps(result, default=str)[:300]}")
            messages.append(
                {
                    "role": "tool",
                    "name": name,
                    "content": json.dumps(result, default=str)[:8000],
                }
            )
    else:
        print("\n[agent] ⚠️ Alcanzado el número máximo de iteraciones.")


# ============================================
# MODO DE USO
# ============================================


async def main() -> None:
    """Punto de entrada. Soporta dos modos:

    1. Modo MCP server (por defecto): expone Blender como servidor MCP stdio.
       Cualquier cliente MCP (Claude Desktop, etc.) puede conectarse.

    2. Modo agente Ollama: si se pasa un argumento, lanza el bucle agentic
       directamente con ese prompt.
    """
    if len(sys.argv) > 1 and sys.argv[1] != "--mcp":
        # Modo agente Ollama.
        task = " ".join(sys.argv[1:])
        await run_agent_loop(task)
        return

    # Modo MCP server (stdio).
    print(
        f"[bridge] Servidor MCP stdio iniciado. Blender en {BLENDER_HOST}:{BLENDER_PORT}.",
        file=sys.stderr,
    )
    print(
        f"[bridge] Conecta desde Claude Desktop, Cursor, etc. con config MCP apuntando a este script.",
        file=sys.stderr,
    )
    async with stdio_server() as (read_stream, write_stream):
        await server.run(
            read_stream, write_stream, server.create_initialization_options()
        )


if __name__ == "__main__":
    asyncio.run(main())
